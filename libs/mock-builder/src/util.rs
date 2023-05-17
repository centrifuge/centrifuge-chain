use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub struct TypeSignature(String);

impl TypeSignature {
	pub fn new<I, O>() -> TypeSignature {
		Self(format!(
			"{}->{}",
			std::any::type_name::<I>(),
			std::any::type_name::<O>(),
		))
	}
}

impl fmt::Display for TypeSignature {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}
