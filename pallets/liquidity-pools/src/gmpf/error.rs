use scale_info::prelude::string::{String, ToString};
use serde::{de, ser};
use sp_std::fmt::{self, Display};

pub type Result<T> = sp_std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
	Message(String),
	EnumSize,
	Unimplemented,
	UnknownSize,
	Eof,
}

impl ser::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::Message(msg.to_string())
	}
}

impl de::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::Message(msg.to_string())
	}
}

impl Display for Error {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let msg = match self {
			Error::Message(msg) => msg,
			Error::EnumSize => "enum variant size too large to serialize(> 255)",
			Error::Unimplemented => "unimplemented serialization",
			Error::UnknownSize => "sequence size is not known",
			Error::Eof => "End of file",
		};

		formatter.write_str(msg)
	}
}

impl ser::StdError for Error {}
