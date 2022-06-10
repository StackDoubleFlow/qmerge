use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    parenthesized, parse_macro_input, token, Error, FnArg, Ident, LitStr, PatType, ReturnType,
    Token, Type,
};

struct Sig {
    ident: Ident,
    params: Punctuated<FnArg, Token![,]>,
    return_ty: ReturnType,
}

impl Parse for Sig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: Token![fn] = input.parse()?;
        let ident = input.parse()?;

        let content;
        let _: token::Paren = parenthesized!(content in input);
        let params = Punctuated::parse_terminated(&content)?;

        let return_ty = if input.peek(Token![->]) {
            input.parse()?
        } else {
            ReturnType::Default
        };
        let _: Token![;] = input.parse()?;

        Ok(Sig {
            ident,
            params,
            return_ty,
        })
    }
}

struct CodegenProxy {
    to: Option<LitStr>,
    input: Sig,
    param_names: Vec<Ident>,
}

impl CodegenProxy {
    fn new(to: Option<LitStr>, input: Sig) -> Self {
        let param_names = input
            .params
            .iter()
            .enumerate()
            .map(|(i, _)| Ident::new(&format!("param{}", i), Span::call_site()))
            .collect();
        Self {
            to,
            input,
            param_names,
        }
    }

    fn param_types(&self) -> Result<Vec<&Type>, Error> {
        self.input
            .params
            .iter()
            .map(|param| match param {
                FnArg::Receiver(_) => Err(Error::new_spanned(
                    param,
                    "Proxy function cannot have `self` parameter",
                )),
                FnArg::Typed(PatType { ty, .. }) => Ok(&**ty),
            })
            .collect()
    }

    fn game_fn_ty(&self) -> Result<TokenStream2, Error> {
        let params = self.param_types()?;
        let return_ty = &self.input.return_ty;
        Ok(quote! {
            fn (#(#params,)*) #return_ty
        })
    }

    fn proxy_sig(&self) -> Result<TokenStream2, Error> {
        let ident = &self.input.ident;
        let param_types = self.param_types()?;
        let param_names = &self.param_names;
        let return_ty = &self.input.return_ty;
        Ok(quote! {
            pub extern "C" fn #ident(#(#param_names: #param_types,)*) #return_ty
        })
    }

    fn proxy_body(&self) -> Result<TokenStream2, Error> {
        let game_fn_ty = self.game_fn_ty()?;
        let proxy_to = match &self.to {
            Some(lit) => lit.clone(),
            None => LitStr::new(&self.input.ident.to_string(), self.input.ident.span()),
        };
        let param_names = &self.param_names;
        Ok(quote! {
            static GAME_FN: SyncLazy<#game_fn_ty> = SyncLazy::new(|| {
                let ptr = xref::get_symbol(#proxy_to).unwrap();
                unsafe { transmute(ptr) }
            });
            GAME_FN(#(#param_names,)*)
        })
    }

    fn proxy_fn(&self) -> Result<TokenStream2, Error> {
        let proxy_sig = self.proxy_sig()?;
        let proxy_body = self.proxy_body()?;
        Ok(quote! {
            #[no_mangle]
            #proxy_sig {
                #proxy_body
            }
        })
    }
}

#[proc_macro_attribute]
pub fn proxy_codegen_api(attr: TokenStream, item: TokenStream) -> TokenStream {
    let arg = if attr.is_empty() {
        None
    } else {
        Some(parse_macro_input!(attr as LitStr))
    };
    let input = parse_macro_input!(item as Sig);

    match CodegenProxy::new(arg, input).proxy_fn() {
        Ok(ts) => ts.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
