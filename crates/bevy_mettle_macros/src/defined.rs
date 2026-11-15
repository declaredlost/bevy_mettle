//! Self-defining types: `#[effect(..)]`, `#[ability(..)]` and `#[curve(..)]`,
//! each key a call to the builder method of the same name.
//!
//! `lasting = 4.0` is `.lasting(4.0)`, `infinite` is `.infinite()`, and
//! `flat::<Life>(-5.0)` is `.flat::<Life>(-5.0)`: so every key is a method,
//! documented once, and a key that isn't one fails to compile as a method that
//! isn't. `name = ".."` names the type instead, and `define = path` hands the
//! whole definition to a function.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    AngleBracketedGenericArguments, Data, DeriveInput, Error, Expr, Fields, Ident, LitStr, Path,
    Token, parenthesized,
};

use crate::shared::{keyed, snake};

/// What a self-defining derive is for: `#[attr(..)]` on a type implementing
/// `trait_path`, whose `define` takes and gives `data`.
pub(crate) struct Defining<'a> {
    /// The attribute its keys are on: `effect`.
    pub(crate) attr: &'a str,
    /// What it is, for errors: "an effect".
    pub(crate) what: &'a str,
    /// The trait implemented.
    pub(crate) trait_path: TokenStream,
    /// The type `define` takes and gives.
    pub(crate) data: TokenStream,
    /// The derive's crate.
    pub(crate) krate: TokenStream,
    /// The kind it's keyed as.
    pub(crate) kind: TokenStream,
}

/// One key: `ident`, `ident = value`, or `ident(args)`, each maybe with a
/// turbofish.
struct Entry {
    ident: Ident,
    generics: Option<AngleBracketedGenericArguments>,
    args: Args,
}

enum Args {
    /// `ident`: called with nothing.
    None,
    /// `ident = value`.
    Value(Expr),
    /// `ident(a, b)`.
    List(Punctuated<Expr, Token![,]>),
}

impl Parse for Entry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.call(syn::ext::IdentExt::parse_any)?;
        let generics = if input.peek(Token![::]) {
            Some(input.parse()?)
        } else {
            None
        };
        let args = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Args::Value(input.parse()?)
        } else if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            Args::List(content.parse_terminated(Expr::parse, Token![,])?)
        } else {
            Args::None
        };
        Ok(Self {
            ident,
            generics,
            args,
        })
    }
}

impl Entry {
    /// `.ident::<G>(args)`, spanned at the key, so a key that isn't a method
    /// is shown where it's written.
    fn call(&self) -> TokenStream {
        let Self {
            ident,
            generics,
            args,
        } = self;
        let args = match args {
            Args::None => quote!(),
            Args::Value(value) => quote!(#value),
            Args::List(list) => quote!(#list),
        };
        quote_spanned!(ident.span()=> .#ident #generics (#args))
    }

    /// The literal `name = ".."` gives.
    fn name(&self) -> syn::Result<LitStr> {
        match &self.args {
            Args::Value(Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(name),
                ..
            })) if self.generics.is_none() => Ok(name.clone()),
            _ => Err(Error::new(self.ident.span(), "expected `name = \"..\"`")),
        }
    }

    /// The function `define = path` gives.
    fn define(&self) -> syn::Result<Path> {
        match &self.args {
            Args::Value(Expr::Path(path)) if self.generics.is_none() => Ok(path.path.clone()),
            _ => Err(Error::new(
                self.ident.span(),
                "expected `define = function`",
            )),
        }
    }
}

pub(crate) fn expand(input: &DeriveInput, defining: &Defining<'_>) -> syn::Result<TokenStream> {
    let Defining {
        attr,
        what,
        trait_path,
        data,
        krate,
        kind,
    } = defining;
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            format!("{what} can't be generic"),
        ));
    }
    let unit = matches!(&input.data, Data::Struct(data) if matches!(data.fields, Fields::Unit));
    if !unit {
        return Err(Error::new_spanned(
            &input.ident,
            format!(
                "{what} is a unit struct, a marker: what changes as it's applied goes on its \
                 spec's context"
            ),
        ));
    }
    let mut name: Option<LitStr> = None;
    let mut define: Option<(Span, Path)> = None;
    let mut calls = Vec::new();
    let mut first_key: Option<Span> = None;
    for found in input
        .attrs
        .iter()
        .filter(|found| found.path().is_ident(attr))
    {
        let entries = found.parse_args_with(Punctuated::<Entry, Token![,]>::parse_terminated)?;
        for entry in entries {
            if entry.ident == "name" {
                if name.is_some() {
                    return Err(Error::new(entry.ident.span(), "`name` is given twice"));
                }
                name = Some(entry.name()?);
            } else if entry.ident == "define" {
                if define.is_some() {
                    return Err(Error::new(entry.ident.span(), "`define` is given twice"));
                }
                define = Some((entry.ident.span(), entry.define()?));
            } else {
                first_key.get_or_insert(entry.ident.span());
                calls.push(entry.call());
            }
        }
    }
    if let (Some((span, _)), Some(_)) = (&define, first_key) {
        return Err(Error::new(
            *span,
            "`define` defines all of it: the other keys go in the function",
        ));
    }
    let ident = &input.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let body = define.map_or_else(
        || quote!(definition #(#calls)*),
        |(_, path)| quote!(#path(definition)),
    );
    let keyed = keyed(ident, krate, kind, &name);
    Ok(quote! {
        impl #trait_path for #ident {
            fn define(definition: #data) -> #data {
                #body
            }
        }
        #keyed
    })
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    fn defining() -> Defining<'static> {
        Defining {
            attr: "effect",
            what: "an effect",
            trait_path: quote!(Effect),
            data: quote!(EffectData),
            krate: quote!(krate),
            kind: quote!(AnyEffect),
        }
    }

    fn expanded(input: DeriveInput) -> String {
        expand(&input, &defining()).unwrap().to_string()
    }

    #[test]
    fn each_key_is_a_builder_call() {
        let expanded = expanded(parse_quote! {
            #[effect(lasting = 4.0, execute_on_apply, flat::<Life>(-5.0))]
            #[effect(tag = Burning)]
            struct Burning;
        });
        assert!(
            expanded.contains(
                "definition . lasting (4.0) . execute_on_apply () . flat :: < Life > (- 5.0) \
                 . tag (Burning)"
            ),
            "{expanded}"
        );
    }

    #[test]
    fn it_is_named_in_snake_case_unless_it_says() {
        let unnamed = expanded(parse_quote! { struct FireBurn; });
        assert!(
            unnamed.contains("const KEY : & 'static str = \"fire_burn\""),
            "{unnamed}"
        );
        let named = expanded(parse_quote! {
            #[effect(name = "fire.burning")]
            struct Burning;
        });
        assert!(
            named.contains("const KEY : & 'static str = \"fire.burning\""),
            "{named}"
        );
    }

    #[test]
    fn define_hands_it_to_a_function() {
        let expanded = expanded(parse_quote! {
            #[effect(define = burning)]
            struct Burning;
        });
        assert!(expanded.contains("burning (definition)"), "{expanded}");
    }

    #[test]
    fn define_is_all_of_it() {
        let input = parse_quote! {
            #[effect(define = burning, lasting = 4.0)]
            struct Burning;
        };
        let error = expand(&input, &defining()).expect_err("an error");
        assert!(error.to_string().contains("defines all of it"), "{error}");
    }

    #[test]
    fn it_is_a_marker() {
        let input = parse_quote! {
            struct Burning { damage: f32 }
        };
        let error = expand(&input, &defining()).expect_err("an error");
        assert!(error.to_string().contains("a marker"), "{error}");
    }
}
