use proc_macro2::{Ident, Span};
use quote::quote;
use syn_rsx::{Node, NodeType};
use crate::widget_macro::widget;

pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed = syn_rsx::parse2(input.into()).unwrap();
    let (begining, inplace) = handle_rsx_nodes(&parsed, Ident::new("__widget", Span::call_site()));
    let transformed = quote! {{
        #begining

        #inplace
    }};
    println!("rsx: \n{}\n\n", transformed);
    transformed.into()
}

// if attrs or children of rsx constructs are expressions in the form context => value,
// we transform them into a closure and call these closures
fn call_context_closure(input: syn::Expr) -> proc_macro2::TokenStream {
    quote! {#input}
}

fn handle_rsx_nodes(input: &Vec<Node>, widget_ident: Ident) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if input.iter().all(|x| x.node_type == NodeType::Element) {
        let (beginning, inplace): (Vec<_>, Vec<_>) = input.iter().map(|x| {
            let name = x.name.as_ref().unwrap();
            let name_str = name.to_string();
            let loc = format!("{}_{}", name.span().start().line, name.span().start().column);

            let mut key = quote! {KeyPart::Widget { name: #name_str, loc: #loc }};
            let widget_ident = Ident::new(&format!("__widget_{}_{}", name_str, loc), Span::call_site());

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
            let (beginning, children_processed) = if x.children.is_empty() {
                (quote! {}, quote! {})
            } else {
                let (begining, inplace) = handle_rsx_nodes(&x.children, widget_ident);
                (begining, quote! {
                    children=#inplace,
                })
            };

            (beginning, quote! {(
                #key,
                std::sync::Arc::new(move |context: Context| {
                    #constructor_ident!(@initial context=context.clone(), #(#processed_attributes,)* #children_processed )
                })
            )})
        }).unzip();

        let to_beginning = quote! {
            #(#beginning)*

            let #widget_ident = Widget {
                children: vec![#(#inplace,)*],
                layout_object: None,
            };
        };
        let inplace = quote! {#widget_ident.clone()};
        (to_beginning, inplace)
    } else if input.len() == 1 {
        let value = input.iter().next().unwrap();
        let value_processed = call_context_closure(value.value.as_ref().unwrap().clone());

        let to_beginning = quote! {};
        let inplace = quote! {#value_processed};
        (to_beginning, inplace)
    } else {
        panic!("each rsx node can either contain n nodes or one block, got {:?}", input);
    }
}
