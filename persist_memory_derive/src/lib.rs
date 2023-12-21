// TODO: call to_persist/revive on fields and make macro for default
// implementation (which is just clone for to_persist and move for
// revive).
// TODO: add Default implementation for persisted. Possibly just using
// the trait system?
extern crate proc_macro;
use quote::quote;
use syn;
use syn::punctuated::Punctuated;
use syn::{Token, DataStruct, DataEnum, Fields, Field, Attribute, Variant};
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

fn get_attr_data(attrs: &[Attribute]) -> syn::Result<Option<PersistAttr>> {
  let mut out: Option<PersistAttr> = None;

  for attr in attrs.iter() {
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

/// Utility for creating formatting based on fields.
///
/// op takes the field, the identifier that should be used as a value
/// (this synchronizes choice if there's no field name), and the PersistAttr
/// if it was annotated.
fn use_fields(
  fields: &Fields,
  op: impl Fn(&Field, Ident, Option<PersistAttr>) -> TokenStream,
) -> syn::Result<TokenStream> {
  fn handle(
    punc: &Punctuated<Field, Token![,]>,
    op: impl Fn(&Field, Ident, Option<PersistAttr>) -> TokenStream,
  ) -> syn::Result<TokenStream> {
    punc.iter()
      .enumerate()
      .map(|(i, field)| -> syn::Result<TokenStream> {
        get_attr_data(&field.attrs.as_slice())
          .map(|opt| {
            let def_ident: Ident = field.ident.clone()
              .unwrap_or_else(|| Ident::new(&format!("v{}", i), Span::call_site()));
            op(field, def_ident, opt)
          })
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
  use_fields(fields, |field, _var, opt_attr| opt_attr.map(|persist_attr| {
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
  inp: DataStruct
) -> syn::Result<TokenStream> {
  // filter out fields that I'll be persisting.
  let field_decl = make_persisted_fields(&inp.fields)?;
  let to_persist_conv = quote! { .to_persist() };
  let revive_conv = quote! { .revive() };
  let to_persist_pattern = use_field_vars(&inp.fields, false, true, None)?;
  let to_persist_fields = use_field_vars(&inp.fields, true, false, Some(&to_persist_conv))?;
  let revive_pattern = use_field_vars(&inp.fields, true, true, None)?;
  let revive_fields = use_field_vars(&inp.fields, false, false, Some(&revive_conv))?;
  /*
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
  */
  let semi = &inp.semi_token;
  let mod_name = Ident::new(&format!("persist_impl_{}", name), Span::call_site());

  let output = quote! {
    mod #mod_name {
      use minicbor;
      use super::*;

      #[derive(::minicbor::Encode, ::minicbor::Decode)]
      pub struct #persisted_ident #field_decl #semi

      impl Default for #persisted_ident {
        fn default() -> #persisted_ident {
          #name::default().to_persist()
        }
      }

      impl ::persist_memory::Persist for #name {
        type Persisted = #persisted_ident;
        fn to_persist(&self) -> Self::Persisted {
          match self {
            #name #to_persist_pattern => #persisted_ident #to_persist_fields
          }
        }

        fn revive(stored: Self::Persisted) -> Self {
          match stored {
            #persisted_ident #revive_pattern => #name #revive_fields
          }
        }
      }
    }
  };
  println!("pretty {}", output);
  Ok(output)
}

fn use_variants<'a>(
  variants: impl Iterator<Item = &'a Variant>,
  op: impl Fn(&'a Variant, Option<PersistAttr>) -> syn::Result<TokenStream>,
) -> syn::Result<TokenStream> {
  variants
    .map(|variant| -> syn::Result<TokenStream> {
      get_attr_data(&variant.attrs.as_slice()).and_then(|opt|
        op(variant, opt)
      )
    })
    .collect::<syn::Result<TokenStream>>()
}

/// Utility function for dealing with generating things that pattern match
/// on fields or construct fields.
///
/// If omit_unpersisted is false, we use `Default::default()` as a replacement
/// for `var`, the default `Ident` provided by `use_fields`.
///
/// `for_pattern` indicates whether this is for a pattern or not, and whether
/// `Default::default()` should be used or the default `var` ident from `use_fields`.
fn use_field_vars(
  fields: &Fields,
  omit_unpersisted: bool,
  for_pattern: bool,
  conv: Option<&TokenStream>,
) -> syn::Result<TokenStream> {
  use_fields(fields, |field, var, opt_attr| {
    if omit_unpersisted && opt_attr.is_none() {
      TokenStream::new()
    } else {
      let prefix = field.ident.as_ref()
        .map_or(TokenStream::new(), |field_name| quote! {
          #field_name :
        });
      let value = if for_pattern {
        quote! { #var }
      } else if let Some(attr) = opt_attr {
        quote! { #var #conv }
      } else {
        quote! { Default::default() }
      };
      quote! {
        #prefix #value,
      }
    }
  })
}

fn for_enum(
  name: Ident,
  persisted_ident: Ident,
  inp: DataEnum,
) -> syn::Result<TokenStream> {
  let enum_decl = use_variants(inp.variants.iter(), |variant, opt_attr| {
    let variant_name = &variant.ident;
    let fields = make_persisted_fields(&variant.fields)?;
    Ok(opt_attr.map(|attr| {
      let n = proc_macro2::Literal::u32_unsuffixed(attr.index);
      quote! {
        #[n(#n)] #variant_name #fields,
      }
    }).unwrap_or_default())
  })?;
  // fn to_persist(&self) -> Self::Persisted {
  //   match self {
  //     A => A,
  //     B(a, b, c,) => B(a, c,)
  //   }
  // }
  let to_persist_body = use_variants(inp.variants.iter(), |variant, opt_attr| {
    let conv = quote! { .to_persist() };
    let variant_name = &variant.ident;
    let pattern = use_field_vars(&variant.fields, false, true, None)?;
    Ok(if opt_attr.is_some() {
      let fields = use_field_vars(&variant.fields, true, false, Some(&conv))?;
      quote! {
        #name::#variant_name #pattern => #persisted_ident::#variant_name #fields,
      }
    } else {
      quote! {
        #name::#variant_name #pattern => #name::default().to_persist(),
      }
    })
  })?;
  // NOTE: there's some kind of co-object with the way the pattern used for
  // structs inverts for enum variants. Oh, I guess that's just sum types being
  // co-products.
  // fn revive(stored: Self::Persisted) -> Self {
  //   match stored {
  //     A => A
  //     B(a, c,) => B(a, Default::default(), c,)
  //   }
  // }
  let revive_body = use_variants(inp.variants.iter(), |variant, opt_attr| {
    opt_attr.map_or(Ok(TokenStream::new()), |attr| {
      let conv = quote! { .revive() };
      let pattern = use_field_vars(&variant.fields, true, true, None)?;
      let variant_name = &variant.ident;
      let fields = use_field_vars(&variant.fields, false, false, Some(&conv))?;
      Ok(quote! {
        #persisted_ident :: #variant_name #pattern => #name::#variant_name #fields,
      })
    })
  })?;
  let mod_name = Ident::new(&format!("persist_impl_{}", name), Span::call_site());
  // TODO: for to_persist I might need to add an option to have it add `.clone()`
  let output = quote! {
    mod #mod_name {
      use minicbor;
      use persist_memory;
      use super::*;
      #[derive(::minicbor::Encode, ::minicbor::Decode)]
      pub enum #persisted_ident { #enum_decl }

      impl Default for #persisted_ident {
        fn default() -> #persisted_ident {
          #name::default().to_persist()
        }
      }

      impl ::persist_memory::Persist for #name {
        type Persisted = #persisted_ident;
        fn to_persist(&self) -> Self::Persisted {
          match self {
            #to_persist_body
          }
        }

        fn revive(stored: Self::Persisted) -> Self {
          match stored {
            #revive_body
          }
        }
      }
    }
  };
  println!("pretty {}", output);
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
    syn::Data::Enum(data) => for_enum(name, persisted_ident, data),
    syn::Data::Union(u) => {
      let msg = "deriving `PersistMemory` for a `union` is not supported";
      Err(syn::Error::new(u.union_token.span, msg))
    }
  };

  let stream = TokenStream::from(result.unwrap_or_else(|e| e.to_compile_error()));
  stream.into()
}
