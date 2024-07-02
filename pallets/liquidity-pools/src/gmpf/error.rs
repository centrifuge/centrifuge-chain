use scale_info::prelude::string::ToString;
use serde::{de, ser};
use sp_std::{
	fmt::{self, Display},
	vec::Vec,
};

pub type Result<T> = sp_std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
	Message(Vec<u8>),
	EnumSize,
	Unimplemented,
	UnknownSize,
	Eof,
}

impl ser::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::Message(msg.to_string().into_bytes())
	}
}

impl de::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::Message(msg.to_string().into_bytes())
	}
}

impl Display for Error {
	fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		let msg = match self {
			Error::Message(msg) => sp_std::str::from_utf8(&msg).unwrap_or(""),
			Error::EnumSize => "enum variant size too large to serialize(> 255)",
			Error::Unimplemented => "unimplemented serialization",
			Error::UnknownSize => "sequence size is not known",
			Error::Eof => "End of file",
		};

		formatter.write_str(msg)
	}
}

impl ser::StdError for Error {}
