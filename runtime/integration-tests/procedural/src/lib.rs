use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Expr, ItemFn};

/// Test the function against different runtimes
///
/// ```rust,ignore
/// use crate::generic::config::Runtime;
///
/// #[test_runtimes([development, altair, centrifuge])]
/// fn foo<T: Runtime> {
///     // Your test here...
/// }
/// ```
/// You can test all runtimes also as:
/// ```rust,ignore
/// use crate::generic::config::Runtime;
///
/// #[test_runtimes(all)]
/// fn foo<T: Runtime> {
///     // Your test here...
/// }
/// ```
///
/// You can test for fudge support adding the bound:
/// ```rust,ignore
/// use crate::generic::{config::Runtime, envs::fudge_env::FudgeSupport};
///
/// #[test_runtimes(all)]
/// fn foo<T: Runtime + FudgeSupport> {
///     // Your test here...
/// }
/// ```
///
/// For the following command: `cargo test -p runtime-integration-tests foo`,
/// it will generate the following output:
///
/// ```text
/// test generic::foo::altair ... ok
/// test generic::foo::development ... ok
/// test generic::foo::centrifuge ... ok
/// ```
///
/// Available input for the argument is:
/// - Any combination of `development`, `altair`, `centrifuge` inside `[]`.
/// - The world `all`.
#[proc_macro_attribute]
pub fn test_runtimes(args: TokenStream, input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(args as Expr);
	let func = parse_macro_input!(input as ItemFn);

	let func_name = &func.sig.ident;

	quote! {
		crate::__test_for_runtimes!(#args, #func_name);
		#func
	}
	.into()
}

/// Wrapper over test_runtime to print the output
#[proc_macro_attribute]
pub fn __dbg_test_runtimes(args: TokenStream, input: TokenStream) -> TokenStream {
	let tokens = test_runtimes(args, input);
	let file = syn::parse_file(&tokens.to_string()).unwrap();

	println!("{}", prettyplease::unparse(&file));

	TokenStream::default()
}
