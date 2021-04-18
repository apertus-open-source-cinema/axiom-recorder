mod rsx;
mod widget;

use quote::quote;
use syn::parse_macro_input;

use syn::{parse_quote, ExprCall};

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
    parsed.args.push(parse_quote! {__context.clone()});
    (quote! {#parsed}).into()
}
