use proc_macro::TokenStream;
use syn::{parse_macro_input, LitStr, ItemFn};
use quote::quote;

// #[proc_macro_attribute]
// pub fn proxy_codegen_api(attr: TokenStream, item: TokenStream) -> TokenStream {
//     let arg = parse_macro_input!(attr as LitStr);
//     let input = parse_macro_input!(item as ItemFn);
    
//     let param_types = input.sig.inputs;

//     quote! {

//     }
// }

// fn create_codegen_proxy
