mod rsx;
mod widget;

use quote::quote;
use syn::parse_macro_input;

use quote::ToTokens;
use syn::{parse_quote, spanned::Spanned, ExprCall};

#[proc_macro]
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream { rsx::rsx(input) }

#[proc_macro]
pub fn toplevel_rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let rsx_macro_output: proc_macro2::TokenStream = rsx::rsx(input).into();
    let transformed = quote! {
        |__context: Context| {
            #rsx_macro_output
        }
    };
    transformed.into()
}
#[proc_macro_attribute]
pub fn widget(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    widget::widget(args, item)
}

#[proc_macro]
pub fn hook(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut parsed: ExprCall = parse_macro_input!(input);
    let span = parsed.func.span();
    let name = parsed.func.to_token_stream().to_string();
    let loc = format!("{}:{}", span.start().line, span.start().column);
    parsed.args.push(parse_quote! {__context.enter_hook(#name, #loc)});
    (quote! {#parsed}).into()
}

#[proc_macro]
pub fn color(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let string = input.to_string();
    let trimmed = string.trim_start_matches("# ");
    let str_to_float =
        |s| i64::from_str_radix(s, 16).unwrap() as f32 / 16u32.pow(s.len() as u32) as f32;
    match trimmed.len() {
        6 => {
            let r = str_to_float(&trimmed[0..2]);
            let g = str_to_float(&trimmed[2..4]);
            let b = str_to_float(&trimmed[4..6]);
            (quote! {
                Color {
                    color: palette::rgb::Rgb {
                        red: #r,
                        green: #g,
                        blue: #b,
                        standard: core::marker::PhantomData,
                    },
                    alpha: 1.0
                }
            })
            .into()
        }
        _ => {
            unimplemented!()
        }
    }
}
