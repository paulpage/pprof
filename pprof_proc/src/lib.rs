use proc_macro::TokenStream;

#[proc_macro]
pub fn time(item: TokenStream) -> TokenStream {
    format!("let _p = pprof::block!({});", item).parse().unwrap()
}
