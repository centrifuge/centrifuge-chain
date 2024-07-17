use serde::{
	de::{self, DeserializeSeed, EnumAccess, IntoDeserializer, SeqAccess, VariantAccess, Visitor},
	Deserialize,
};

use super::error::{Error, Result};

struct Deserializer<'de> {
	input: &'de [u8],
}

impl<'de> Deserializer<'de> {
	fn from_slice(input: &'de [u8]) -> Self {
		Deserializer { input }
	}
}

pub fn from_slice<'a, T: Deserialize<'a>>(s: &'a [u8]) -> Result<T> {
	let mut deserializer = Deserializer::from_slice(s);
	T::deserialize(&mut deserializer)
}

impl<'de> Deserializer<'de> {
	fn consume<const N: usize>(&mut self) -> Result<&[u8; N]> {
		match self.input.split_first_chunk::<N>() {
			Some((consumed, remaining)) => {
				self.input = remaining;
				Ok(consumed)
			}
			None => Err(Error::Eof),
		}
	}
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
	type Error = Error;

	fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("any".into()))
	}

	fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_bool(self.consume::<1>()?[0] != 0)
	}

	fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_i8(i8::from_be_bytes(*self.consume::<1>()?))
	}

	fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_i16(i16::from_be_bytes(*self.consume::<2>()?))
	}

	fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_i32(i32::from_be_bytes(*self.consume::<4>()?))
	}

	fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_i64(i64::from_be_bytes(*self.consume::<8>()?))
	}

	fn deserialize_i128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_i128(i128::from_be_bytes(*self.consume::<16>()?))
	}

	fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_u8(u8::from_be_bytes(*self.consume::<1>()?))
	}

	fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_u16(u16::from_be_bytes(*self.consume::<2>()?))
	}

	fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_u32(u32::from_be_bytes(*self.consume::<4>()?))
	}

	fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_u64(u64::from_be_bytes(*self.consume::<8>()?))
	}

	fn deserialize_u128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_u128(u128::from_be_bytes(*self.consume::<16>()?))
	}

	fn deserialize_f32<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("f32".into()))
	}

	fn deserialize_f64<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("f64".into()))
	}

	fn deserialize_char<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("char".into()))
	}

	fn deserialize_str<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("str".into()))
	}

	fn deserialize_string<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("string".into()))
	}

	fn deserialize_bytes<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("bytes".into()))
	}

	fn deserialize_byte_buf<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("byte_buf".into()))
	}

	fn deserialize_option<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("option".into()))
	}

	fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_unit()
	}

	fn deserialize_unit_struct<V: Visitor<'de>>(
		self,
		_name: &'static str,
		visitor: V,
	) -> Result<V::Value> {
		visitor.visit_unit()
	}

	fn deserialize_newtype_struct<V: Visitor<'de>>(
		self,
		_name: &'static str,
		visitor: V,
	) -> Result<V::Value> {
		visitor.visit_newtype_struct(self)
	}

	fn deserialize_seq<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("seq".into()))
	}

	fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
		visitor.visit_seq(SeqDeserializer(self, len))
	}

	fn deserialize_tuple_struct<V: Visitor<'de>>(
		self,
		_name: &'static str,
		len: usize,
		visitor: V,
	) -> Result<V::Value> {
		self.deserialize_tuple(len, visitor)
	}

	fn deserialize_map<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("map".into()))
	}

	fn deserialize_struct<V: Visitor<'de>>(
		self,
		_name: &'static str,
		fields: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value> {
		self.deserialize_tuple(fields.len(), visitor)
	}

	fn deserialize_enum<V: Visitor<'de>>(
		self,
		_name: &'static str,
		_variants: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value> {
		visitor.visit_enum(self)
	}

	fn deserialize_identifier<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("indentifier".into()))
	}

	fn deserialize_ignored_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		Err(Error::Unimplemented("ignored_any".into()))
	}

	fn is_human_readable(&self) -> bool {
		false
	}
}

impl<'de, 'a> EnumAccess<'de> for &'a mut Deserializer<'de> {
	type Error = Error;
	type Variant = Self;

	fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self::Variant)> {
		let index = self.consume::<1>()?[0];
		Ok((seed.deserialize(index.into_deserializer())?, self))
	}
}

impl<'de, 'a> VariantAccess<'de> for &'a mut Deserializer<'de> {
	type Error = Error;

	fn unit_variant(self) -> Result<()> {
		Ok(())
	}

	fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
		DeserializeSeed::deserialize(seed, self)
	}

	fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
		de::Deserializer::deserialize_tuple(self, len, visitor)
	}

	fn struct_variant<V: Visitor<'de>>(
		self,
		fields: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value> {
		de::Deserializer::deserialize_tuple(self, fields.len(), visitor)
	}
}

struct SeqDeserializer<'a, 'de>(&'a mut Deserializer<'de>, usize);

impl<'de, 'a> SeqAccess<'de> for SeqDeserializer<'a, 'de> {
	type Error = Error;

	fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
		if self.1 > 0 {
			self.1 -= 1;
			let value = de::DeserializeSeed::deserialize(seed, &mut *self.0)?;
			Ok(Some(value))
		} else {
			Ok(None)
		}
	}

	fn size_hint(&self) -> Option<usize> {
		Some(self.1)
	}
}
