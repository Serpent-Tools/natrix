//! Implement function for formatting macros.

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, TokenStreamExt, quote};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::{Expr, LitStr, Token};

/// Input to the macro
struct Input {
    /// Maybe a `move` beforehand
    maybe_move: Option<TokenStream>,
    /// The closure arguments that will be prefixed
    closure: TokenStream,
    /// The string literal
    string_literal: LitStr,
    /// The expression arguments
    expressions: Punctuated<Expr, Token![,]>,
}

impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let maybe_move = if input.peek(Token![move]) {
            Some(input.parse()?)
        } else {
            None
        };

        input.parse::<Token![|]>()?;
        let mut closure = TokenStream::new();
        while !input.peek(Token![|]) {
            let token = input.parse::<TokenTree>()?;
            closure.append(token);
        }
        input.parse::<Token![|]>()?;

        let string_literal = input.parse::<LitStr>()?;
        input.parse::<Token![,]>()?;
        let expressions = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;

        Ok(Self {
            maybe_move,
            closure,
            string_literal,
            expressions,
        })
    }
}

/// The kind of section
enum Kind {
    /// Just text (already encoded as a string litral)
    Text(TokenStream),
    /// Pop a value from the arguments
    NeedsValues,
}

impl Input {
    /// Parse the literal string and produce the sections.
    fn parse_string(&self) -> syn::Result<Vec<Kind>> {
        let mut result = Vec::new();
        let mut current_string = String::new();

        let mut in_bracket = false;

        for char in self.string_literal.value().chars() {
            match char {
                '{' => {
                    if in_bracket {
                        in_bracket = false;
                        current_string.push('{');
                    } else {
                        in_bracket = true;
                    }
                }
                '}' => {
                    if in_bracket {
                        result.push(Kind::Text(current_string.to_token_stream()));
                        result.push(Kind::NeedsValues);
                        current_string.clear();
                        in_bracket = false;
                    } else {
                        current_string.push('}');
                    }
                }
                char => {
                    if in_bracket {
                        return Err(syn::Error::new_spanned(
                            &self.string_literal,
                            "format brackets can not contain content.",
                        ));
                    }
                    current_string.push(char);
                }
            }
        }

        result.push(Kind::Text(current_string.to_token_stream()));
        Ok(result)
    }
}

/// actual implementation of `format_elements!`
pub fn format_elements(raw_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let raw_clone = raw_input.clone();
    let input = syn::parse_macro_input!(raw_input as Input);
    let segments = match input.parse_string() {
        Ok(segments) => segments,
        Err(error) => {
            return error.to_compile_error().into();
        }
    };

    let mut expressions = input.expressions.iter();
    let closure = input.closure;
    let maybe_move = input.maybe_move;

    let elements = segments
        .into_iter()
        .map(|kind| match kind {
            Kind::Text(value) => Ok(quote!(::natrix::macro_ref::Element::render(#value))),
            Kind::NeedsValues => {
                let Some(expression) = expressions.next() else {
                    return Err(syn::Error::new_spanned(
                        TokenStream::from(raw_clone.clone()),
                        "Expected more arguments",
                    )
                    .into_compile_error()
                    .into());
                };

                Ok(quote!(::natrix::macro_ref::Element::render(#maybe_move |#closure| #expression)))
            }
        })
        .collect::<Result<Vec<_>, _>>();
    let elements = match elements {
        Ok(elements) => elements,
        Err(error) => return error,
    };

    let result = quote!([#(#elements),*]);

    result.into()
}
