//! Expansion of `#[derive(StableHash)]`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Field, Fields, Ident, Index, Member, Result, parse_quote};

pub(crate) fn expand(mut input: DeriveInput) -> Result<TokenStream> {
    for param in input.generics.type_params_mut() {
        param.bounds.push(parse_quote!(::strata_kernel::StableHash));
    }
    let body = match &input.data {
        Data::Struct(data) => {
            let hashes = field_hashes(&data.fields, |member| quote!(&self.#member))?;
            quote!(#(#hashes)*)
        }
        Data::Enum(data) => {
            let mut arms = Vec::new();
            for (index, variant) in data.variants.iter().enumerate() {
                let name = &variant.ident;
                let index = u32::try_from(index)
                    .map_err(|_| syn::Error::new_spanned(variant, "too many variants"))?;
                let bindings = bindings(&variant.fields);
                let hashes = field_hashes(&variant.fields, |member| {
                    let binding = binding_ident(member);
                    quote!(#binding)
                })?;
                let pattern = pattern(&variant.fields, &bindings);
                arms.push(quote! {
                    Self::#name #pattern => {
                        ::strata_kernel::StableHash::stable_hash(&#index, hasher);
                        #(#hashes)*
                    }
                });
            }
            quote!(match self { #(#arms)* })
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "StableHash cannot be derived for unions",
            ));
        }
    };
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::strata_kernel::StableHash for #name #ty_generics #where_clause {
            #[allow(unused_variables)]
            fn stable_hash(&self, hasher: &mut ::strata_kernel::StableHasher) {
                #body
            }
        }
    })
}

/// One `stable_hash` call per non-skipped field; `access` turns a member into a `&T` expression.
fn field_hashes(
    fields: &Fields,
    access: impl Fn(&Member) -> TokenStream,
) -> Result<Vec<TokenStream>> {
    let mut hashes = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if is_skipped(field)? {
            continue;
        }
        let value = access(&member(field, index));
        hashes.push(quote!(::strata_kernel::StableHash::stable_hash(#value, hasher);));
    }
    Ok(hashes)
}

fn is_skipped(field: &Field) -> Result<bool> {
    let mut skip = false;
    for attr in field
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("stable_hash"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                skip = true;
                Ok(())
            } else {
                Err(meta.error("expected `skip`"))
            }
        })?;
    }
    Ok(skip)
}

fn member(field: &Field, index: usize) -> Member {
    match &field.ident {
        Some(ident) => Member::Named(ident.clone()),
        None => Member::Unnamed(Index::from(index)),
    }
}

fn binding_ident(member: &Member) -> Ident {
    match member {
        Member::Named(ident) => format_ident!("field_{}", ident),
        Member::Unnamed(index) => Ident::new(&format!("field_{}", index.index), Span::call_site()),
    }
}

fn bindings(fields: &Fields) -> Vec<(Member, Ident)> {
    fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let member = member(field, i);
            let ident = binding_ident(&member);
            (member, ident)
        })
        .collect()
}

fn pattern(fields: &Fields, bindings: &[(Member, Ident)]) -> TokenStream {
    match fields {
        Fields::Named(_) => {
            let parts = bindings.iter().map(|(m, b)| quote!(#m: #b));
            quote!({ #(#parts),* })
        }
        Fields::Unnamed(_) => {
            let names = bindings.iter().map(|(_, b)| b);
            quote!(( #(#names),* ))
        }
        Fields::Unit => quote!(),
    }
}
