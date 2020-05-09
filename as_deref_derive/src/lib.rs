use itertools::Itertools as _;
use quote::quote;
use std::convert::TryInto as _;
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned as _, Attribute, Data, DataStruct,
    DeriveInput, Field, Fields, FieldsUnnamed, Generics, Ident, Index, Meta, MetaList, NestedMeta,
    Visibility,
};

#[proc_macro_derive(AsDeref, attributes(as_deref))]
pub fn as_deref(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(fields),
            ..
        }) => as_deref_for_tuple_struct(&input.vis, &input.ident, &input.generics, fields),
        _ => todo!(),
    }
    .unwrap_or_else(|err| err.to_compile_error())
    .into()
}

fn as_deref_for_tuple_struct(
    vis: &Visibility,
    ident: &Ident,
    generics: &Generics,
    fields: &FieldsUnnamed,
) -> syn::Result<proc_macro2::TokenStream> {
    enum Kind {
        Deref,
    }

    let pairs = fields
        .unnamed
        .iter()
        .enumerate()
        .map(|(index, Field { attrs, vis, ty, .. })| {
            let kinds = attrs
                .iter()
                .flat_map(Attribute::parse_meta)
                .filter(|m| m.path().get_ident().map_or(false, |i| i == "as_deref"))
                .map(|meta| {
                    if let Meta::List(MetaList { nested, .. }) = meta {
                        let kind = nested
                            .iter()
                            .exactly_one()
                            .ok()
                            .and_then(|nested| match nested {
                                NestedMeta::Meta(meta) => Some(meta),
                                NestedMeta::Lit(_) => None,
                            })
                            .and_then(|meta| match meta {
                                Meta::Path(path) => path.get_ident(),
                                Meta::List(_) | Meta::NameValue(_) => None,
                            })
                            .ok_or_else(|| {
                                syn::Error::new(nested.span(), "Expected 1 identifier")
                            })?;

                        match &*kind.to_string() {
                            "deref" => Ok(Kind::Deref),
                            _ => Err(syn::Error::new(kind.span(), "Expected `deref`")),
                        }
                    } else {
                        Err(syn::Error::new(meta.span(), "Expected `as_deref($ident)`"))
                    }
                })
                .collect::<syn::Result<Vec<_>>>()?;

            let index = Index {
                index: index.try_into().expect("the length should be < 2^32"),
                span: proc_macro2::Span::call_site(),
            };
            let expr = quote!(&self.#index);

            match &*kinds {
                [] => Ok((quote!(#vis &'__as_deref #ty), expr)),
                [Kind::Deref] => Ok((
                    quote!(#vis &'__as_deref <#ty as ::core::ops::Deref>::Target),
                    expr,
                )),
                [..] => Err(syn::Error::new(
                    ty.span(),
                    "Multiple `#[as_deref(_)]` options",
                )),
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let fields = pairs.iter().map(|(f, _)| f);
    let exprs = pairs.iter().map(|(_, e)| e);

    let mut extended_generics = generics.clone();
    extended_generics
        .params
        .insert(0, parse_quote!('__as_deref));
    extended_generics.lt_token = Some(parse_quote!(<));
    extended_generics.gt_token = Some(parse_quote!(>));

    let (_, original_ty_generics, where_clause) = generics.split_for_impl();
    let (impl_generics, extended_ty_generics, _) = extended_generics.split_for_impl();

    let target = Ident::new(&format!("{}AsDeref", ident), proc_macro2::Span::call_site());

    Ok(quote! {
        #vis struct #target #impl_generics (#(#fields),*) #where_clause;

        impl #impl_generics ::as_deref::AsDeref for &'__as_deref #ident #original_ty_generics #where_clause {
            type Target = #target #extended_ty_generics;

            fn as_deref(self) -> Self::Target {
                #target(#(#exprs),*)
            }
        }
    })
}
