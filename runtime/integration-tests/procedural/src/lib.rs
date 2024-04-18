use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn test_runtimes(args: TokenStream, input: TokenStream) -> TokenStream {
	dbg!(args.to_string());
	dbg!(input.to_string());
	input
}

#[proc_macro_attribute]
pub fn test_runtimes_with_fudge(args: TokenStream, input: TokenStream) -> TokenStream {
	dbg!(args.to_string());
	dbg!(input.to_string());
	input
}
