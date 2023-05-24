use frame_support::StorageHasher;

use super::util::TypeSignature;

/// Absolute string identification of function.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FunctionLocation(String);

impl FunctionLocation {
	/// Creates a location for the function which created the given closure used
	/// as a locator
	pub fn from<F: Fn()>(_: F) -> Self {
		let location = std::any::type_name::<F>();
		let location = &location[..location.len() - "::{{closure}}".len()];

		// Remove generic attributes from signature if it has any
		let location = location
			.ends_with('>')
			.then(|| {
				let mut count = 0;
				for (i, c) in location.chars().rev().enumerate() {
					if c == '>' {
						count += 1;
					} else if c == '<' {
						count -= 1;
						if count == 0 {
							return location.split_at(location.len() - i - 1).0;
						}
					}
				}
				panic!("Expected '<' symbol to close '>'");
			})
			.unwrap_or(location);

		Self(location.into())
	}

	/// Normalize the location, allowing to identify the function
	/// no matter if it belongs to a trait or not.
	pub fn normalize(self) -> Self {
		let (path, name) = self.0.rsplit_once("::").expect("always ::");
		let path = path
			.strip_prefix('<')
			.map(|trait_path| trait_path.split_once(" as").expect("always ' as'").0)
			.unwrap_or(path);

		Self(format!("{}::{}", path, name))
	}

	/// Remove the prefix from the function name.
	pub fn strip_name_prefix(self, prefix: &str) -> Self {
		let (path, name) = self.0.rsplit_once("::").expect("always ::");
		let name = name.strip_prefix(prefix).unwrap_or_else(|| {
			panic!(
				"Function '{name}' should have a '{prefix}' prefix. Location: {}",
				self.0
			)
		});

		Self(format!("{}::{}", path, name))
	}

	/// Add a representation of the function input and output types
	pub fn append_type_signature<I, O>(self) -> Self {
		Self(format!("{}:{}", self.0, TypeSignature::new::<I, O>()))
	}

	/// Generate a hash of the location
	pub fn hash<Hasher: StorageHasher>(&self) -> Hasher::Output {
		Hasher::hash(self.0.as_bytes())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const PREFIX: &str = "mock_builder::location::tests";

	trait TraitExample {
		fn method() -> FunctionLocation;
		fn generic_method<A: Into<i32>>(_: impl Into<u32>) -> FunctionLocation;
	}

	struct Example;

	impl Example {
		fn mock_method() -> FunctionLocation {
			FunctionLocation::from(|| ())
		}

		fn mock_generic_method<A: Into<i32>>(_: impl Into<u32>) -> FunctionLocation {
			FunctionLocation::from(|| ())
		}
	}

	impl TraitExample for Example {
		fn method() -> FunctionLocation {
			FunctionLocation::from(|| ())
		}

		fn generic_method<A: Into<i32>>(_: impl Into<u32>) -> FunctionLocation {
			FunctionLocation::from(|| ())
		}
	}

	#[test]
	fn function_location() {
		assert_eq!(
			Example::mock_method().0,
			format!("{PREFIX}::Example::mock_method")
		);

		assert_eq!(
			Example::mock_generic_method::<i8>(0u8).0,
			format!("{PREFIX}::Example::mock_generic_method")
		);

		assert_eq!(
			Example::method().0,
			format!("<{PREFIX}::Example as {PREFIX}::TraitExample>::method")
		);

		assert_eq!(
			Example::generic_method::<i8>(0u8).0,
			format!("<{PREFIX}::Example as {PREFIX}::TraitExample>::generic_method")
		);
	}

	#[test]
	fn normalized_function_location() {
		assert_eq!(
			Example::mock_method().normalize().0,
			format!("{PREFIX}::Example::mock_method")
		);

		assert_eq!(
			Example::method().normalize().0,
			format!("{PREFIX}::Example::method")
		);
	}

	#[test]
	fn striped_function_location() {
		assert_eq!(
			Example::mock_method().strip_name_prefix("mock_").0,
			format!("{PREFIX}::Example::method")
		);
	}

	#[test]
	fn appended_type_signature() {
		assert_eq!(
			Example::mock_method().append_type_signature::<i8, u8>().0,
			format!("{PREFIX}::Example::mock_method:i8->u8")
		);
	}
}
