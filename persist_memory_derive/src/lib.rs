extern crate proc_macro;
use quote::quote;
use syn;
use syn::punctuated::Punctuated;
use syn::{Token, DataStruct, Fields, Field};
use syn::parse::{Parse, ParseBuffer};
use proc_macro2::{Ident, Span, TokenStream};

struct PersistAttr {
  index: u32,
  /// The path used for #[cbor(with = "path::to::mod")].
  coder_path: Option<syn::LitStr>,
}

/// Parse an index to be used for labeling it for minicbor.
fn parse_index(stream: &ParseBuffer<'_>) -> syn::Result<u32> {
  let n = syn::LitInt::parse(stream)?;
  n.base10_parse()
    .map_err(|_| syn::Error::new(n.span(), "expected `u32` value"))
}

fn get_attr_data(field: &Field) -> syn::Result<Option<PersistAttr>> {
  let mut out: Option<PersistAttr> = None;

  for attr in field.attrs.iter() {
    if attr.meta.path().is_ident("persist") {
      let pattr = attr.parse_args_with(|stream: &'_ ParseBuffer<'_>| -> syn::Result<PersistAttr> {
        let index = parse_index(stream)?;
        let coder_path = if stream.peek(Token![,]) {
          <Token![,]>::parse(stream)?;
          Some(stream.parse::<syn::LitStr>()?)
        } else {
          None
        };
        Ok(PersistAttr {
          index, coder_path
        })
      })?;
      // throw an error if we have two
      if out.is_some() {
        return Err(syn::Error::new_spanned(attr, "two #[persist(...)] attributes"));
      } else {
        out = Some(pattr);
      }
    }
  }

  Ok(out)
}

fn use_fields(
  fields: &Fields,
  mut op: impl FnMut(&Field, Option<PersistAttr>) -> TokenStream,
) -> syn::Result<TokenStream> {
  fn handle(
    punc: &Punctuated<Field, Token![,]>,
    mut op: impl FnMut(&Field, Option<PersistAttr>) -> TokenStream,
  ) -> syn::Result<TokenStream> {
    punc.iter()
      .map(|field| -> syn::Result<TokenStream> {
        get_attr_data(field).map(|opt|
          op(field, opt)
        )
      })
      .collect::<syn::Result<TokenStream>>()
  }
  Ok(match fields {
    Fields::Named(data) => {
      let new_fields = handle(&data.named, op)?;
      quote! { { #new_fields } }
    },
    Fields::Unnamed(data) => {
      let new_fields = handle(&data.unnamed, op)?;
      quote! { ( #new_fields ) }
    },
    Fields::Unit => TokenStream::new(),
  })
}

fn make_persisted_fields(
  fields: &Fields,
) -> syn::Result<TokenStream> {
  use_fields(fields, |field, opt_attr| opt_attr.map(|persist_attr| {
    let ty = &field.ty;
    let n = persist_attr.index;
    let with = persist_attr.coder_path.map(|s| quote! {
      #[cbor(with = #s)]
    });
    let attr = quote! {
      #[n(#n)] #with
    };
    match &field.ident {
      Some(ident) => quote! {
        #attr #ident : #ty,
      },
      None => quote! {
        #attr #ty,
      },
    }
  }).unwrap_or_default())
}

fn for_struct(
  name: Ident,
  persisted_ident: Ident,
  inp: DataStruct,
) -> syn::Result<TokenStream> {
  // filter out fields that I'll be persisting.
  let field_decl = make_persisted_fields(&inp.fields)?;
  let to_persist_fields = use_fields(&inp.fields, |field, opt_attr| {
    let name = &field.ident;
    opt_attr.map(|attr| quote! {
      #name: self.#name,
    }).unwrap_or_default()
  })?;
  let revive_fields = use_fields(&inp.fields, |field, opt_attr| {
    let name = &field.ident;
    opt_attr.map_or(quote!{
      #name: Default::default(),
    }, |attr| quote! {
      #name: stored.#name,
    })
  })?;

  let output = quote! {
    #[derive(::minicbor::Encode, ::minicbor::Decode)]
    pub struct #persisted_ident #field_decl
    impl ::persist_memory::Persist for #name {
      type Persisted = #persisted_ident;
      fn to_persist(&self) -> Self::Persisted {
        #persisted_ident #to_persist_fields
      }

      fn revive(stored: Self::Persisted) -> Self {
        #name #revive_fields
      }
    }
  };
  Ok(output)
}

#[proc_macro_derive(Persist, attributes(persist))]
pub fn derive_persist_memory(
  tokens: proc_macro::TokenStream
) -> proc_macro::TokenStream {
  let input = syn::parse_macro_input!(tokens as syn::DeriveInput);

  let name = input.ident;
  // name of the new persisted identifier
  let persisted_ident = Ident::new(&format!("Persist{}", name),
    Span::call_site());
  // for now we won't worry about generics. We'll hand write instances for
  // those, especially given how annoying they are.
  let result = match input.data {
    syn::Data::Struct(data) => for_struct(name, persisted_ident, data),
    syn::Data::Enum(data) => {
      todo!("not implemented yet");
    },
    syn::Data::Union(u) => {
      let msg = "deriving `PersistMemory` for a `union` is not supported";
      Err(syn::Error::new(u.union_token.span, msg))
    }
  };

  let stream = TokenStream::from(result.unwrap_or_else(|e| e.to_compile_error()));
  stream.into()
}

#[cfg(test)]
mod tests {
  use super::*;
  // TODO: tests
}
