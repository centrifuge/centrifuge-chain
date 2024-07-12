use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, Expr, ItemFn, Token};

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
/// You can also ignore them:
/// ```rust,ignore
/// use crate::generic::config::Runtime;
///
/// #[test_runtimes([development, altair, centrifuge], ignore = "reason")]
/// fn foo<T: Runtime> {
///     // Your test here...
/// }
///
/// #[test_runtimes(all, ignore = "reason")]
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
	let mut args: Punctuated<Expr, Token![,]> =
		parse_macro_input!(args with Punctuated::parse_terminated);

	let runtimes = args
		.get(0)
		.expect("expect 'all' or a list of runtimes")
		.clone();

	let ignore = args.get(1).clone();

	let func = parse_macro_input!(input as ItemFn);
	let func_name = &func.sig.ident;

	let test_for_runtimes = match ignore {
		Some(ignore) => quote!(crate::__test_for_runtimes!(#runtimes, #func_name, #ignore);),
		None => quote!(crate::__test_for_runtimes!(#runtimes, #func_name);),
	};

	quote! {
		#test_for_runtimes
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
