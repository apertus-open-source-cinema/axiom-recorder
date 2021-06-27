use bind_match::bind_match;
use core::result::{Result, Result::Ok};
use proc_macro2::{Ident, Literal, Span};
use quote::{quote, ToTokens};
use std::collections::{HashMap, HashSet};
use syn::{
    parse::{Parse, ParseStream, Parser},
    parse_quote,
    punctuated::Punctuated,
    Expr,
    FnArg,
    ItemFn,
    Pat,
    Token,
    Type,
};

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

pub fn widget(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let parsed: Result<ItemFn, _> = syn::parse2(item.into());
    let mut function = parsed.unwrap();

    let function_ident = function.sig.ident.clone();
    let macro_ident = Ident::new(
        &format!("__{}_constructor", function_ident.clone()),
        Span::call_site()
    );

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
        quote! { let #ident = move || #value }
    });

    let args_with: HashSet<_> = parsed_args.clone().into_iter().map(|x| x.ident.to_string()).collect();

    let match_arm = arg_names.clone().map(|x| {
        let arg_names_comma_dollar = arg_names_comma_dollar.clone();
        let arg_names_comma_ident = arg_names_comma_ident.clone();

        let dummy_function_ident = Ident::new(&format!("return_second_{}", x), Span::call_site());
        let dummy_function_type = arg_types.get(&x.to_string()).unwrap();
        let dummy_function = quote! {
                // this is needed to be able to use the default argument with the correct type &
                // mute unusesd warnings
                #[allow(non_snake_case, unused)]
                fn #dummy_function_ident(
                    _first: impl Fn() -> #dummy_function_type + Clone,
                    second: impl Fn() -> #dummy_function_type + Clone
                ) -> impl Fn() -> #dummy_function_type + Clone {
                    second
                }
            };
        let value = if args_with.contains(&x.to_string()) {
            quote! {#dummy_function_ident($#x, move || $value)}
        } else {
            quote! {move || $value}
        };

        quote! {
            (@parse [#arg_names_comma_ident] #x = $value:expr,$($rest:tt)*) => {
                #dummy_function
                let $#x = #value;
                #macro_ident!(@parse [#arg_names_comma_dollar] $($rest)*);
            };
        }
    });

    let return_type = function.sig.output.clone().to_token_stream().to_string().replace("-> ", "");
    let num_args = (0..arg_names.len()).map(|i| Literal::usize_unsuffixed(i));

    let inner = {
        let num_args = num_args.clone();
        match return_type.as_str() {
            "Widget" => {
                quote! {WidgetInner::Composed { widget: parking_lot::Mutex::new(#function_ident(#(clone_without_deref(&args_tuple.#num_args),)* __context.clone()))}}
            }
            "WidgetInner" => quote! { #function_ident(#(clone_without_deref(&args_tuple.#num_args),)* __context.clone()) },
            t => unimplemented!(
                "widget functions must return either Widget or WidgetInner not {}",
                t
            ),
        }
    };

    let context_ident = Ident::new("__context", Span::call_site());
    function.sig.inputs.push(parse_quote! {#context_ident: Context});

    let transformed = quote! {
        #[macro_export]
        macro_rules! #macro_ident {
            (@initial __context=$context:expr, $($args:tt)*) => {
                {
                    #(#initializers;)*
                    #macro_ident!(@parse [#arg_names_comma_1] $($args)*);

                    let __context = $context.clone();
                    Widget::Unevaluated(UnevaluatedWidget { key: __context.key.clone(), gen: std::sync::Arc::new(move || {
                        fn clone_without_deref<T: Clone>(x: &T) -> T { x.clone() }
                        let args_tuple = (#({let __context = __context.clone(); (#arg_names.clone())()},)*);
                        let args_state_value = StateValue::new(__context.clone(), "args");
                        fn constrain_type_helper<T> (_a: &StateValue<T>, _b: &T) {}
                        constrain_type_helper(&args_state_value, &args_tuple);
                        args_state_value.update_now(args_tuple.clone());

                        let to_return = { #inner };

                        StateValue::new(__context.clone(), "used").set_sneaky_now(__context.used.clone());
                        to_return
                    }) })
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
    println!(" {}", transformed.clone());
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
