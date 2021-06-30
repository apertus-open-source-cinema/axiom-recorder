use bind_match::bind_match;
use core::result::{Result, Result::Ok};
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use std::collections::{HashMap, HashSet};
use syn::{
    parse::{Parse, ParseStream, Parser},
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

// allows for kwarg-style calling of functions
pub fn widget(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // parse the function
    let parsed: Result<ItemFn, _> = syn::parse2(item.into());
    let function = parsed.unwrap();

    let return_type = function.sig.output.clone().to_token_stream().to_string().replace("-> ", "");
    assert_eq!(return_type, "Widget");

    // parse & format the default arguments
    let parser = Punctuated::<AttributeParameter, Token![,]>::parse_terminated;
    let parsed_args = parser.parse(args).unwrap();

    let function_ident = function.sig.ident.clone();
    let macro_ident = Ident::new(
        &format!("__{}_constructor", function_ident.clone()),
        Span::call_site()
    );

    let match_arms: Vec<_> = {
        let args_with: HashSet<_> = parsed_args.clone().into_iter().map(|x| x.ident.to_string()).collect();
        let arg_names_comma_dollar = {
            let arg_names = get_arg_names(&function).clone();
            quote! {#($#arg_names,)*}
        };
        let arg_names_comma_ident = {
            let arg_names = get_arg_names(&function).clone();
            quote! {#($#arg_names:ident,)*}
        };

        get_arg_names(&function).iter().map(|x| {
            let arg_names_comma_dollar = arg_names_comma_dollar.clone();
            let arg_names_comma_ident = arg_names_comma_ident.clone();

            let dummy_function_ident = Ident::new(&format!("return_second_{}", x), Span::call_site());
            let dummy_function = quote! {
                    // this is needed to be able to use the default argument with the correct type &
                    // mute unusesd warnings
                    #[allow(non_snake_case, unused)]
                    fn #dummy_function_ident<T>(_first: T, second: T) -> T {
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
        }).collect()
    };

    let initial_arm = {
        let initializers = parsed_args.clone().into_iter().map(|x| {
            let ident = x.ident;
            let value = x.expr;
            quote! { let #ident = move || #value }
        });
        let arg_names_comma = {
            let arg_names = get_arg_names(&function);
            quote! {#(#arg_names,)*}
        };
        let function_call = quote! {
            #function_ident(#arg_names_comma)
        };


        quote! {
            (@initial context=$context:expr, $($args:tt)*) => {
                {
                    #(#initializers;)*
                    #macro_ident!(@parse [#arg_names_comma] $($args)*);

                    #function_call
                }
            };
        }
    };

    let constructor_macro = {
        let arg_names_comma_ident = {
            let arg_names = get_arg_names(&function).clone();
            quote! {#($#arg_names:ident,)*}
        };

        quote! {
            #[macro_export]
            macro_rules! #macro_ident {
                #initial_arm

                #(#match_arms)*

                (@parse [#arg_names_comma_ident] ) => { };
            }

            // we do this to have correct scoping of the macro. It should not just be placed at the
            // crate root but rather at the path of the original function.
            pub use #macro_ident as #macro_ident;
        }
    };

    let transformed_function = transform_function_args_to_context(function);

    let transformed = quote! {
        #constructor_macro

        #transformed_function
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

// adds the function arguments to the context as a `Listenable` and listen on it for
// partial re-evaluation.
fn transform_function_args_to_context(function: ItemFn) -> proc_macro2::TokenStream {
    let function_clone = function.clone();
    let ItemFn { attrs, vis, sig, block } = function;
    let stmts = &block.stmts;
    let context_string = get_arg_types(&function_clone)
        .iter()
        .filter(|(_, ty)| ty.to_token_stream().to_string().replace("-> ", "") == "Context")
        .next().unwrap().0.to_string();
    let context_ident = Ident::new(&context_string, Span::call_site());
    let arg_names = get_arg_names(&function_clone);
    let function_transformed = quote! {
        #(#attrs)* #vis #sig {
            let args_listenable = #context_ident.listenable_key(#context_ident.widget_local.key.with(KeyInner::sideband("args")));
            #context_ident.shout(&args_listenable, (#(#arg_names.clone(), )*));
            #context_ident.listen(&args_listenable);

            let to_return = {
                #(#stmts)*
            };

            mem::forget(context);  // we consume the context here to prevent the other widgets from giving it out
            to_return
        }
    };
    function_transformed
}

fn get_arg_names(function: &ItemFn) -> Vec<Ident> {
    function.clone().sig.inputs.into_iter().map(|arg| {
        let pat_type = bind_match!(arg, FnArg::Typed(x) => x).unwrap();
        let pat_ident = bind_match!(*pat_type.pat, Pat::Ident(x) => x).unwrap();
        pat_ident.ident
    }).collect()
}

fn get_arg_types(function: &ItemFn) -> HashMap<String, Box<Type>> {
    function
        .clone()
        .sig
        .inputs
        .into_iter()
        .map(|arg| {
            let pat_type = bind_match!(arg, FnArg::Typed(x) => x).unwrap();
            let pat_ident = bind_match!(*pat_type.pat, Pat::Ident(x) => x).unwrap();

            (pat_ident.ident.to_string(), pat_type.ty)
        })
        .collect()
}
