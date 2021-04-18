use proc_macro2::{Ident, Span};
use quote::quote;
use syn_rsx::{Node, NodeType};

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
        quote! {#v}
    } else {
        println!("{}", input.node_type);
        unimplemented!();
    }
}
