use bind_match::bind_match;
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use std::collections::{HashMap, HashSet};
use syn::{
    parse::{Parse, ParseStream, Parser},
    parse_macro_input,
    parse_quote,
    punctuated::Punctuated,
    Expr,
    ExprCall,
    FnArg,
    ItemFn,
    Pat,
    Token,
    Type,
};
use syn_rsx::{Node, NodeType};

#[proc_macro]
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut parsed = syn_rsx::parse2(input.into()).unwrap();

    assert_eq!(parsed.len(), 1);
    let transformed = handle_rsx_node(parsed.pop().unwrap());
    transformed.into()
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
        } else if input.children.iter().any(|x| x.node_type == NodeType::Element) {
            let children: Vec<_> = input.children.into_iter().map(|x| handle_rsx_node(x)).collect();
            if children.len() == 1 {
                let child = &children[0];
                quote! { children=#child.into(), }
            } else {
                quote! { children=vec![#(#children),*], }
            }
        } else {
            assert_eq!(input.children.len(), 1);
            let child = input.children[0].value.as_ref().unwrap();
            quote! { children=#child, }
        };


        quote! {
            #constructor_ident!(@initial #(#processed_attributes,)* #children_processed __context=__context.enter_widget(#key),)
        }
    } else if input.node_type == NodeType::Block {
        let v = input.value.unwrap();
        quote!{#v}
    } else {
        println!("{}", input.node_type);
        unimplemented!();
    }
}


// a helper to parse the parameters to the widget proc macro attribute
// we cant use the syn AttributeArgs here because it can only handle literals
// and we want expressions (e.g. for closures)
#[derive(Debug, Clone)]
struct AttributeParameter {
    ident: Ident,
    expr: Expr,
}
impl Parse for AttributeParameter {
    fn parse(input: ParseStream<'_>) -> syn::parse::Result<Self> {
        let ident = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let expr = input.parse::<Expr>()?;

        Ok(AttributeParameter { ident, expr })
    }
}

#[proc_macro_attribute]
pub fn widget(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let parsed: Result<ItemFn, _> = syn::parse2(item.into());
    let mut function = parsed.unwrap();

    let function_ident = function.sig.ident.clone();
    let macro_ident =
        Ident::new(&format!("__{}_constructor", function.sig.ident.clone()), Span::call_site());

    let context_ident = Ident::new("__context", Span::call_site());
    function.sig.inputs.push(parse_quote! {#context_ident: Context});

    let arg_names = function.clone().sig.inputs.into_iter().map(|arg| {
        let pat_type = bind_match!(arg, FnArg::Typed(x) => x).unwrap();
        let pat_ident = bind_match!(*pat_type.pat, Pat::Ident(x) => x).unwrap();
        pat_ident.ident
    });

    let arg_types: HashMap<String, Box<Type>> = function
        .clone()
        .sig
        .inputs
        .into_iter()
        .map(|arg| {
            let pat_type = bind_match!(arg, FnArg::Typed(x) => x).unwrap();
            let pat_ident = bind_match!(*pat_type.pat, Pat::Ident(x) => x).unwrap();

            (pat_ident.ident.to_string(), pat_type.ty)
        })
        .collect();

    let arg_names_comma = {
        let arg_names = arg_names.clone();
        quote! {#(#arg_names,)*}
    };
    let arg_names_comma_dollar = {
        let arg_names = arg_names.clone();
        quote! {#($#arg_names,)*}
    };
    let arg_names_comma_ident = {
        let arg_names = arg_names.clone();
        quote! {#($#arg_names:ident,)*}
    };

    let arg_names_comma_1 = arg_names_comma.clone();
    let arg_names_comma_2 = arg_names_comma.clone();
    let arg_names_comma_ident_1 = arg_names_comma_ident.clone();

    // parse & format the default arguments
    let parser = Punctuated::<AttributeParameter, Token![,]>::parse_terminated;
    let parsed_args = parser.parse(args).unwrap();

    let initializers = parsed_args.clone().into_iter().map(|x| {
        let ident = x.ident;
        let value = x.expr;
        quote! { let #ident = #value }
    });

    let args_with: HashSet<_> = parsed_args.clone().into_iter().map(|x| {
        x.ident.to_string()
    }).collect();

    let match_arm = arg_names.clone().map(|x| {
        let arg_names_comma_dollar = arg_names_comma_dollar.clone();
        let arg_names_comma_ident = arg_names_comma_ident.clone();

        let dummy_function_ident = Ident::new(&format!("return_second_{}", x), Span::call_site());
        let dummy_function_type = arg_types.get(&x.to_string()).unwrap();
        let dummy_function = quote! {
                // this is needed to be able to use the default argument with the correct type &
                // mute unusesd warnings
                fn #dummy_function_ident(_first: #dummy_function_type, second: #dummy_function_type) -> #dummy_function_type {
                    second
                }
            };
        let value = if args_with.contains(&x.to_string()) {
            quote! {#dummy_function_ident($#x, $value)}
        } else {
            quote! {$value}
        };

        quote! {
            (@parse [#arg_names_comma_ident]  #x = $value:expr,$($rest:tt)*) => {
                #dummy_function
                let $#x = #value;
                #macro_ident!(@parse [#arg_names_comma_dollar] $($rest)*);
            };
        }
    });

    let transformed = quote! {
        #[macro_export]
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

        // we do this to have correct scoping of the macro. It should not just be placed at the
        // crate root
        pub use #macro_ident as #macro_ident;

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
