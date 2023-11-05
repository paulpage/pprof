use proc_macro::TokenStream;

#[cfg(feature = "profile")]
#[proc_macro]
pub fn time(item: TokenStream) -> TokenStream {
    format!("let _p = pprof::block!({});", item).parse().unwrap()
}

#[cfg(not(feature = "profile"))]
#[proc_macro]
pub fn time(_item: TokenStream) -> TokenStream {
    TokenStream::default()
}
