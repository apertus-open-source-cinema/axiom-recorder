use syn_rsx::{parse2, Node, NodeType};
use quote::{quote, ToTokens};
use proc_macro2::{Ident, Span};
use syn::{ItemFn, FnArg, Meta, parse_macro_input, NestedMeta, parse_quote};
use std::collections::HashMap;

#[proc_macro]
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut parsed = parse2(input.into()).unwrap();

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

        let children: Vec<_> = input.children.into_iter().map(|x| handle_rsx_node(x)).collect();
        let children_processed = if !children.is_empty() {
            quote! { children=vec![#(#children),*], }
        } else { quote! {} };
        quote! {
            #constructor_ident!(@initial #(#processed_attributes,)* #children_processed __context=__context.enter_widget(#key),);
        }
    } else {
        quote! {}
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
        FnArg::Typed(v) => v.pat,
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


    // get & format the default arguments
    type AttributeArgs = Vec<NestedMeta>;
    let meta = parse_macro_input!(args as AttributeArgs);
    let arg_name_hashmap: HashMap<_, _> = arg_names.clone().into_iter().map(|x| (format!("{}", x.to_token_stream()), x)).collect();
    let initializers = meta.into_iter().map(|x| {
        match x {
            NestedMeta::Meta(Meta::NameValue(x)) => {
                let ident = arg_name_hashmap.get(&format!("{}", x.path.get_ident().unwrap()));
                let value = x.lit;
                quote! { let #ident = #value }
            },
            _ => unimplemented!()
        }
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
pub fn context(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let to_return = quote! {
        __context
    };
    to_return.into()
}
