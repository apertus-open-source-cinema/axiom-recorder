use proc_macro2::{Ident, Span};
use quote::quote;
use syn_rsx::{Node, NodeType};
use syn::__private::ToTokens;

pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed = syn_rsx::parse2(input.into()).unwrap();
    let transformed = handle_rsx_nodes(&parsed);
    transformed.into()
}

// if attrs or children of rsx constructs are closures that get the context as an argument,
// we call these closures. TODO: find a way to explicitly prevent this? maybe addidional braces?
fn call_context_closure(input: syn::Expr) -> proc_macro2::TokenStream {
    match input {
        syn::Expr::Closure(c) => {
            let c = c.into_token_stream();
            quote!{{
                fn constrain_type<T>(x: impl Fn(Context) -> T) -> (impl Fn(Context) -> T) { x }
                constrain_type(#c)(context.clone())
            }}
        },
        syn::Expr::Block(b) => {
            if b.block.stmts.len() == 1 {
                call_context_closure(match (&b.block.stmts[0]).clone() {
                    syn::Stmt::Expr(e) => e,
                    _ => unimplemented!(),
                })
            } else {
                let value = b.into_token_stream();
                quote! { #value }
            }
        }
        other => {
            let value = other.into_token_stream();
            quote! { #value }
        },
    }
}

fn handle_rsx_nodes(input: &Vec<Node>) -> proc_macro2::TokenStream {
    if input.iter().all(|x| x.node_type == NodeType::Element) {
        let mapped: Vec<_> = input.iter().map(|x| {
            let name = x.name.as_ref().unwrap();
            let name_str = name.to_string();
            let loc = format!("{}:{}", name.span().start().line, name.span().start().column);

            let mut key = quote! {KeyPart::Widget { name: #name_str, loc: #loc }};

            let constructor_ident = Ident::new(&format!("__{}_constructor", name), Span::call_site());
            let mut processed_attributes = vec![];
            for attribute in &x.attributes {
                let name = attribute.name.as_ref().unwrap();
                let value = call_context_closure(attribute.value.as_ref().unwrap().clone());
                if name.to_string() == "key" {
                    key = quote! {KeyPart::WidgetKey { name: #name_str, loc: #loc, key: KeyPart::calculate_hash(#value) }}
                } else {
                    processed_attributes.push(quote! {#name=#value});
                }
            }
            let children_processed = if x.children.is_empty() {
                quote! {}
            } else {
                handle_rsx_nodes(&x.children)
            };

            quote! {(
                #key,
                Box::new(|context: Context| {
                    #constructor_ident!(@initial context=context.clone(), #(#processed_attributes,)* #children_processed )
                })
            )}
        }).collect();

        quote! {
            Widget::Node(vec![#(#mapped,)*])
        }
    } else if input.len() == 1 {
        let value = input.iter().next().unwrap();
        let value_processed = call_context_closure(value.value.as_ref().unwrap().clone());
        quote! {#value_processed}
    } else {
        panic!("each rsx node can either contain n nodes or one block, got {:?}", input);
    }
}
