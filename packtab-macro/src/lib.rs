extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Expr, Ident, LitInt, Token, Visibility, Type};

/// Input syntax:
/// ```text
/// packtab_macro::pack_table! {
///     pub fn lookup(u: usize) -> u8 {
///         data: [1, 2, 3, 4, 5],
///         default: 0,
///         compression: 1.0,
///     }
/// }
/// ```
struct PackTableInput {
    vis: Visibility,
    fn_name: Ident,
    _arg_name: Ident,
    _ret_type: Type,
    data: Vec<i64>,
    default: i64,
    compression: f64,
}

impl Parse for PackTableInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis: Visibility = input.parse()?;
        input.parse::<Token![fn]>()?;
        let fn_name: Ident = input.parse()?;

        let paren_content;
        syn::parenthesized!(paren_content in input);
        let arg_name: Ident = paren_content.parse()?;
        paren_content.parse::<Token![:]>()?;
        let _arg_type: Type = paren_content.parse()?;

        input.parse::<Token![->]>()?;
        let ret_type: Type = input.parse()?;

        let brace_content;
        syn::braced!(brace_content in input);

        // data: [...]
        let data_ident: Ident = brace_content.parse()?;
        if data_ident != "data" {
            return Err(syn::Error::new_spanned(data_ident, "expected 'data'"));
        }
        brace_content.parse::<Token![:]>()?;

        let bracket_content;
        syn::bracketed!(bracket_content in brace_content);
        let mut data = Vec::new();
        while !bracket_content.is_empty() {
            if bracket_content.peek(Token![-]) {
                bracket_content.parse::<Token![-]>()?;
                let lit: LitInt = bracket_content.parse()?;
                data.push(-(lit.base10_parse::<i64>()?));
            } else {
                let lit: LitInt = bracket_content.parse()?;
                data.push(lit.base10_parse::<i64>()?);
            }
            if bracket_content.peek(Token![,]) {
                bracket_content.parse::<Token![,]>()?;
            }
        }
        brace_content.parse::<Token![,]>()?;

        // default: N
        let default_ident: Ident = brace_content.parse()?;
        if default_ident != "default" {
            return Err(syn::Error::new_spanned(default_ident, "expected 'default'"));
        }
        brace_content.parse::<Token![:]>()?;
        let default = if brace_content.peek(Token![-]) {
            brace_content.parse::<Token![-]>()?;
            let lit: LitInt = brace_content.parse()?;
            -(lit.base10_parse::<i64>()?)
        } else {
            let lit: LitInt = brace_content.parse()?;
            lit.base10_parse::<i64>()?
        };

        // Optional: compression: F
        let mut compression = 1.0f64;
        if brace_content.peek(Token![,]) {
            brace_content.parse::<Token![,]>()?;
            if !brace_content.is_empty() {
                let comp_ident: Ident = brace_content.parse()?;
                if comp_ident != "compression" {
                    return Err(syn::Error::new_spanned(comp_ident, "expected 'compression'"));
                }
                brace_content.parse::<Token![:]>()?;
                let expr: Expr = brace_content.parse()?;
                compression = match &expr {
                    Expr::Lit(lit) => match &lit.lit {
                        syn::Lit::Float(f) => f.base10_parse::<f64>()?,
                        syn::Lit::Int(i) => i.base10_parse::<f64>()?,
                        _ => return Err(syn::Error::new_spanned(lit, "expected number")),
                    },
                    _ => return Err(syn::Error::new_spanned(expr, "expected number literal")),
                };
                if brace_content.peek(Token![,]) {
                    brace_content.parse::<Token![,]>()?;
                }
            }
        }

        Ok(PackTableInput {
            vis,
            fn_name,
            _arg_name: arg_name,
            _ret_type: ret_type,
            data,
            default,
            compression,
        })
    }
}

/// Pack a table of integers into compact multi-level lookup tables at compile time.
///
/// # Example
///
/// ```text
/// packtab_macro::pack_table! {
///     pub fn lookup(u: usize) -> u8 {
///         data: [1, 2, 3, 4, 5, 6, 7, 8],
///         default: 0,
///     }
/// }
/// ```
#[proc_macro]
pub fn pack_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as PackTableInput);

    let (info, best_idx) = packtab::pack_table(&input.data, input.default, input.compression);
    let code_str = packtab::generate(
        &info,
        best_idx,
        &input.fn_name.to_string(),
        packtab::codegen::Language::Rust { unsafe_access: false },
    );

    // Adjust visibility: replace "pub(crate) fn name_get" with user's visibility + name.
    let vis_str = match &input.vis {
        Visibility::Public(_) => "pub",
        Visibility::Inherited => "",
        _ => "pub(crate)",
    };

    let fn_name_str = input.fn_name.to_string();
    let adjusted = code_str.replace(
        &format!("pub(crate) fn {}_get", fn_name_str),
        &format!("{} fn {}", vis_str, fn_name_str),
    );
    // Replace internal references to name_get with just name
    let adjusted = adjusted.replace(
        &format!("{}_get", fn_name_str),
        &fn_name_str,
    );

    let generated: proc_macro2::TokenStream = adjusted
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse generated code: {}\n\nCode:\n{}", e, adjusted));

    let output = quote! {
        #[allow(dead_code, unused_parens, non_upper_case_globals)]
        #generated
    };

    output.into()
}
