use serde::{ser, Serialize};
use sp_std::vec::Vec;

use super::error::{Error, Result};

struct Serializer {
	output: Vec<u8>,
}

pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>> {
	let mut serializer = Serializer {
		output: Vec::default(),
	};
	value.serialize(&mut serializer)?;
	Ok(serializer.output)
}

impl<'a> ser::Serializer for &'a mut Serializer {
	type Error = Error;
	type Ok = ();
	type SerializeMap = Self;
	type SerializeSeq = Self;
	type SerializeStruct = Self;
	type SerializeStructVariant = Self;
	type SerializeTuple = Self;
	type SerializeTupleStruct = Self;
	type SerializeTupleVariant = Self;

	fn serialize_bool(self, v: bool) -> Result<()> {
		self.output.push(v as u8);
		Ok(())
	}

	fn serialize_i8(self, v: i8) -> Result<()> {
		self.output.push(v as u8);
		Ok(())
	}

	fn serialize_i16(self, v: i16) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_i32(self, v: i32) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_i64(self, v: i64) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_i128(self, v: i128) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_u8(self, v: u8) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_u16(self, v: u16) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_u32(self, v: u32) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_u64(self, v: u64) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_u128(self, v: u128) -> Result<()> {
		self.output.extend(&v.to_be_bytes());
		Ok(())
	}

	fn serialize_f32(self, _v: f32) -> Result<()> {
		Err(Error::Unimplemented)
	}

	fn serialize_f64(self, _v: f64) -> Result<()> {
		Err(Error::Unimplemented)
	}

	fn serialize_char(self, _v: char) -> Result<()> {
		Err(Error::Unimplemented)
	}

	fn serialize_str(self, _v: &str) -> Result<()> {
		Err(Error::Unimplemented)
	}

	fn serialize_bytes(self, v: &[u8]) -> Result<()> {
		self.output.extend(v);
		Ok(())
	}

	fn serialize_none(self) -> Result<()> {
		Err(Error::Unimplemented)
	}

	fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<()> {
		Err(Error::Unimplemented)
	}

	fn serialize_unit(self) -> Result<()> {
		Ok(())
	}

	fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
		Ok(())
	}

	fn serialize_unit_variant(
		self,
		_name: &'static str,
		variant_index: u32,
		_variant: &'static str,
	) -> Result<()> {
		let index = u8::try_from(variant_index).map_err(|_| Error::EnumSize)?;
		self.output.push(index);
		Ok(())
	}

	fn serialize_newtype_struct<T>(self, _name: &'static str, _value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		Ok(())
	}

	fn serialize_newtype_variant<T>(
		self,
		_name: &'static str,
		variant_index: u32,
		_variant: &'static str,
		value: &T,
	) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		let index = u8::try_from(variant_index).map_err(|_| Error::EnumSize)?;
		self.output.push(index);
		value.serialize(self)
	}

	fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
		Err(Error::Unimplemented)
	}

	fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
		Ok(self)
	}

	fn serialize_tuple_struct(
		self,
		_name: &'static str,
		_len: usize,
	) -> Result<Self::SerializeTupleStruct> {
		Ok(self)
	}

	fn serialize_tuple_variant(
		self,
		_name: &'static str,
		variant_index: u32,
		_variant: &'static str,
		_len: usize,
	) -> Result<Self::SerializeTupleVariant> {
		let index = u8::try_from(variant_index).map_err(|_| Error::EnumSize)?;
		self.output.push(index);
		Ok(self)
	}

	fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
		Err(Error::Unimplemented)
	}

	fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
		Ok(self)
	}

	fn serialize_struct_variant(
		self,
		_name: &'static str,
		variant_index: u32,
		_variant: &'static str,
		_len: usize,
	) -> Result<Self::SerializeStructVariant> {
		let index = u8::try_from(variant_index).map_err(|_| Error::EnumSize)?;
		self.output.push(index);
		Ok(self)
	}
}

impl<'a> ser::SerializeSeq for &'a mut Serializer {
	type Error = Error;
	type Ok = ();

	fn serialize_element<T>(&mut self, value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<()> {
		Ok(())
	}
}

impl<'a> ser::SerializeTuple for &'a mut Serializer {
	type Error = Error;
	type Ok = ();

	fn serialize_element<T>(&mut self, value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<()> {
		Ok(())
	}
}

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
	type Error = Error;
	type Ok = ();

	fn serialize_field<T>(&mut self, value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<()> {
		Ok(())
	}
}

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
	type Error = Error;
	type Ok = ();

	fn serialize_field<T>(&mut self, value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<()> {
		Ok(())
	}
}

impl<'a> ser::SerializeMap for &'a mut Serializer {
	type Error = Error;
	type Ok = ();

	fn serialize_key<T>(&mut self, key: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		key.serialize(&mut **self)
	}

	fn serialize_value<T>(&mut self, value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<()> {
		Ok(())
	}
}

impl<'a> ser::SerializeStruct for &'a mut Serializer {
	type Error = Error;
	type Ok = ();

	fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<()> {
		Ok(())
	}
}

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
	type Error = Error;
	type Ok = ();

	fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
	where
		T: ?Sized + Serialize,
	{
		value.serialize(&mut **self)
	}

	fn end(self) -> Result<()> {
		Ok(())
	}
}
