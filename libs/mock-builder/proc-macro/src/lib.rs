use proc_macro::TokenStream;
use syn::spanned::Spanned;

#[proc_macro_attribute]
pub fn pallet(attr: TokenStream, item: TokenStream) -> TokenStream {
	if !attr.is_empty() {
		let msg = "Invalid pallet macro call: unexpected attribute. Macro call must be \
            bare, such as `#[frame_support::pallet]` or `#[pallet]`";
		let span = proc_macro2::TokenStream::from(attr).span();
		return syn::Error::new(span, msg).to_compile_error().into();
	}

	let syn_item = syn::parse_macro_input!(item as syn::ItemMod);
	println!("{:?}", syn_item.ident.to_string());
	TokenStream::new()
}
