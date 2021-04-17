use syn_rsx::{Node, NodeType};
use quote::{quote};
use proc_macro2::{Ident, Span};
use syn::{ItemFn, FnArg, parse_macro_input, parse_quote, Token, Expr, ExprCall, Pat};
use syn::punctuated::Punctuated;
use syn::parse::{Parser, Parse, ParseStream};

#[proc_macro]
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut parsed = syn_rsx::parse2(input.into()).unwrap();

    assert_eq!(parsed.len(), 1);
    let transformed = handle_rsx_node(parsed.pop().unwrap());
    (quote! { #transformed; }).into()
}
fn handle_rsx_node(input: Node) -> proc_macro2::TokenStream {
    if input.node_type == NodeType::Element {
        let name = input.name.unwrap();
        let key = format!("{}@{}:{}", name, name.span().start().line, name.span().start().column);

        let constructor_ident = Ident::new(&format!("__{}_constructor", name), Span::call_site());
        let processed_attributes = input.attributes.into_iter().map(|x| {
            let name = x.name.unwrap();
            let value = x.value.unwrap();
            quote! {#name=#value}
        });

        let children_processed = if input.children.is_empty() {
            quote! {}
        } else if input.children.iter().all(|x| x.node_type == NodeType::Element) {
            let children: Vec<_> = input.children.into_iter().map(|x| handle_rsx_node(x)).collect();
            quote! { children=vec![#(#children),*], }
        } else {
            assert_eq!(input.children.len(), 1);
            let child = input.children[0].value.as_ref().unwrap();
            quote! { children=#child, }
        };


        quote! {
            #constructor_ident!(@initial #(#processed_attributes,)* #children_processed __context=__context.enter_widget(#key),)
        }
    } else {
        unimplemented!();
    }
}


// a helper to parse the parameters to the widget proc macro attribute
// we cant use the syn AttributeArgs here because it can only handle literals and we want expressions
// (e.g. for closures)
#[derive(Debug)]
struct AttributeParameter {
    ident: Ident,
    expr: Expr,
}
impl Parse for AttributeParameter {
    fn parse(input: ParseStream<'_>) -> syn::parse::Result<Self> {
        let ident = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let expr = input.parse::<Expr>()?;

        Ok(AttributeParameter {
            ident, expr
        })
    }
}

#[proc_macro_attribute]
pub fn widget(args: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed: Result<ItemFn, _> = syn::parse2(item.into());
    let mut function = parsed.unwrap();

    let function_ident = function.sig.ident.clone();
    let macro_ident = Ident::new(&format!("__{}_constructor", function.sig.ident.clone()), Span::call_site());

    let context_ident = Ident::new("__context", Span::call_site());
    function.sig.inputs.push(parse_quote! {#context_ident: Context});

    let arg_names = function.clone().sig.inputs.into_iter().map(|x| { match x {
        FnArg::Typed(v) => match *v.pat {
            Pat::Ident(i) => {i.ident},
            _ => unimplemented!()
        },
        _ => unimplemented!()
    }});

    let arg_names_comma = {
        let arg_names = arg_names.clone();
        quote!{#(#arg_names,)*}
    };
    let arg_names_comma_dollar = {
        let arg_names = arg_names.clone();
        quote!{#($#arg_names,)*}
    };
    let arg_names_comma_ident = {
        let arg_names = arg_names.clone();
        quote!{#($#arg_names:ident,)*}
    };

    let match_arm = arg_names.clone().map(|x| {
        let arg_names_comma_dollar = arg_names_comma_dollar.clone();
        let arg_names_comma_ident = arg_names_comma_ident.clone();
        quote! {
            (@parse [#arg_names_comma_ident]  #x = $value:expr,$($rest:tt)*) => {
                let $#x = $value;
                #macro_ident!(@parse [#arg_names_comma_dollar] $($rest)*);
            };
        }
    });

    let arg_names_comma_1 = arg_names_comma.clone();
    let arg_names_comma_2 = arg_names_comma.clone();
    let arg_names_comma_ident_1 = arg_names_comma_ident.clone();

    // parse & format the default arguments
    let parser = Punctuated::<AttributeParameter, Token![,]>::parse_terminated;
    let parsed_args = parser.parse(args).unwrap();

    let initializers = parsed_args.into_iter().map(|x| {
        let ident = x.ident;
        let value = x.expr;
        quote! { let #ident = #value }
    });

    let transformed = quote! {
        macro_rules! #macro_ident {
            (@initial $($args:tt)*) => {
                {
                    #(#initializers;)*
                    #macro_ident!(@parse [#arg_names_comma_1] $($args)*);
                    #function_ident(#arg_names_comma_2)
                }
            };

            #(#match_arm)*

            (@parse [#arg_names_comma_ident_1] ) => { };
        }

        #function
    };
    transformed.into()
}
// a (simplified) example of the kind of macro this proc macro generates:
/*
macro_rules! button_constructor {
    (@initial $($args:tt)*) => {
        {
            let size = 12.0;
            button_constructor!(@parse [size, text] $($args)*);

            button(text, size)
        }
    };
    (@parse [$size:ident, $text:ident] size = $value:expr,$($rest:tt)*) => {
        let $size = $value;
        button_constructor!(@parse [$size, $text] $($rest)*);
    };
    (@parse [$size:ident, $text:ident] text = $value:expr,$($rest:tt)*) => {
        let $text = $value;
        button_constructor!(@parse [$size, $text] $($rest)*);
    };
    (@parse [$size:ident, $text:ident] ) => { };
}
*/


#[proc_macro]
pub fn hook(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut parsed: ExprCall = parse_macro_input!(input);
    parsed.args.push(parse_quote! {__context.clone()});
    (quote! {#parsed}).into()
}
