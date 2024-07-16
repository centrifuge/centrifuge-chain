use scale_info::prelude::string::{String, ToString};
use serde::{de, ser};
use sp_std::fmt::{self, Display};

pub type Result<T> = sp_std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
	Message(String),
	EnumSize,
	Unimplemented(String),
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
			Error::Message(string) => string.clone(),
			Error::EnumSize => "enum variant size too large to serialize(> 255)".into(),
			Error::Unimplemented(who) => format!("unimplemented '{who}'"),
			Error::UnknownSize => "sequence size is not known".into(),
			Error::Eof => "End of file".into(),
		};

		formatter.write_str(&msg)
	}
}

impl ser::StdError for Error {}
