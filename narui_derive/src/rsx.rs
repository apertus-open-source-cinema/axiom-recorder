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
        let name_str = name.to_string();
        let loc = format!("{}:{}", name.span().start().line, name.span().start().column);

        let mut key = quote! {enter_widget(#name_str, #loc)};

        let constructor_ident = Ident::new(&format!("__{}_constructor", name), Span::call_site());
        let mut processed_attributes = vec![];
        for attribute in input.attributes {
            let name = attribute.name.unwrap();
            let value = attribute.value.unwrap();
            if name.to_string() == "key" {
                key = quote! {enter_widget_key(#name_str, #loc, #value.to_string())}
            } else {
                processed_attributes.push(quote! {#name=#value});
            }
        }

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
            quote! { children=#child.into(), }
        };


        quote! {
            {
                let __context = __context.#key;
                #constructor_ident!(@initial __context=__context.clone(), #(#processed_attributes,)* #children_processed )
            }
        }
    } else if input.node_type == NodeType::Block {
        let v = input.value.unwrap();
        quote! {#v}
    } else {
        println!("{}", input.node_type);
        unimplemented!();
    }
}
