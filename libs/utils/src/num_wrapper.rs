use parity_scale_codec::{
	Compact, CompactAs, CompactRef, Decode, Encode, EncodeAsRef, EncodeLike, HasCompact, Input,
	MaxEncodedLen, Ref, WrapperTypeDecode, WrapperTypeEncode,
};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{
	Bounded, CheckedAdd, CheckedDiv, CheckedMul, CheckedNeg, CheckedRem, CheckedShl, CheckedShr,
	CheckedSub, IntegerSquareRoot, One, Saturating, Zero,
};
use sp_std::{
	cmp::Ordering,
	fmt::{self, Debug},
	marker::PhantomData,
	ops::{
		Add, AddAssign, Deref, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Shl, Shr, Sub,
		SubAssign,
	},
};

/// Type that allows to create different typed numbers with the same inner
/// type:
///
/// ```
/// # use crate::num_wrapper::NumWrapper;
///
/// struct Id1;
/// struct Id2;
///
/// type FooU64 = NumWrapper<u64, Id1>;
/// type BarU64 = NumWrapper<u64, Id2>;
/// ```
#[derive(TypeInfo, Serialize, Deserialize)]
#[scale_info(skip_type_params(T, I))]
pub struct NumWrapper<T, I> {
	pub inner: T,
	_instance: PhantomData<I>,
}

impl<T, I> NumWrapper<T, I> {
	pub const fn from(value: T) -> Self {
		NumWrapper {
			inner: value,
			_instance: PhantomData,
		}
	}
}

/*
impl<T: From<u8>, I> From<u8> for NumWrapper<T, I> {
	fn from(other: u8) -> Self {
		Self::from((other).into())
	}
}

impl<T: From<u16>, I> From<u16> for NumWrapper<T, I> {
	fn from(other: u16) -> Self {
		Self::from((other).into())
	}
}

impl<T: From<u32>, I> From<u32> for NumWrapper<T, I> {
	fn from(other: u32) -> Self {
		Self::from((other).into())
	}
}

impl<T: From<u64>, I> From<u64> for NumWrapper<T, I> {
	fn from(other: u64) -> Self {
		Self::from((other).into())
	}
}

impl<T: From<u128>, I> From<u128> for NumWrapper<T, I> {
	fn from(other: u128) -> Self {
		Self::from((other).into())
	}
}
*/

impl<T: TryInto<u8>, I> TryInto<u8> for NumWrapper<T, I> {
	type Error = T::Error;

	fn try_into(self) -> Result<u8, Self::Error> {
		Ok(self.inner.try_into()?)
	}
}

impl<T: TryInto<u16>, I> TryInto<u16> for NumWrapper<T, I> {
	type Error = T::Error;

	fn try_into(self) -> Result<u16, Self::Error> {
		Ok(self.inner.try_into()?)
	}
}

impl<T: TryInto<u32>, I> TryInto<u32> for NumWrapper<T, I> {
	type Error = T::Error;

	fn try_into(self) -> Result<u32, Self::Error> {
		Ok(self.inner.try_into()?)
	}
}

impl<T: TryInto<u64>, I> TryInto<u64> for NumWrapper<T, I> {
	type Error = T::Error;

	fn try_into(self) -> Result<u64, Self::Error> {
		Ok(self.inner.try_into()?)
	}
}

impl<T: TryInto<u128>, I> TryInto<u128> for NumWrapper<T, I> {
	type Error = T::Error;

	fn try_into(self) -> Result<u128, Self::Error> {
		Ok(self.inner.try_into()?)
	}
}

impl<T: Default, I> Default for NumWrapper<T, I> {
	fn default() -> Self {
		Self::from(T::default())
	}
}

impl<T: Clone, I> Clone for NumWrapper<T, I> {
	fn clone(&self) -> Self {
		Self::from(self.inner.clone())
	}
}

impl<T: Copy, I> Copy for NumWrapper<T, I> {}

impl<T: PartialEq, I> PartialEq for NumWrapper<T, I> {
	fn eq(&self, other: &Self) -> bool {
		self.inner.eq(&other.inner)
	}
}

impl<T: Eq, I> Eq for NumWrapper<T, I> {}

impl<T: PartialOrd, I> PartialOrd for NumWrapper<T, I> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.inner.partial_cmp(&other.inner)
	}
}

impl<T: Ord, I> Ord for NumWrapper<T, I> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.inner.cmp(&other.inner)
	}
}

impl<T: Debug, I> Debug for NumWrapper<T, I> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
		self.inner.fmt(f)
	}
}

impl<T: Add<Output = T>, I> Add for NumWrapper<T, I> {
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		Self::from(self.inner.add(rhs.inner))
	}
}

impl<T: Sub<Output = T>, I> Sub for NumWrapper<T, I> {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		Self::from(self.inner.sub(rhs.inner))
	}
}

impl<T: Mul<Output = T>, I> Mul for NumWrapper<T, I> {
	type Output = Self;

	fn mul(self, rhs: Self) -> Self {
		Self::from(self.inner.mul(rhs.inner))
	}
}

impl<T: Div<Output = T>, I> Div for NumWrapper<T, I> {
	type Output = Self;

	fn div(self, rhs: Self) -> Self {
		Self::from(self.inner.div(rhs.inner))
	}
}

impl<T: Rem<Output = T>, I> Rem for NumWrapper<T, I> {
	type Output = Self;

	fn rem(self, rhs: Self) -> Self {
		Self::from(self.inner.rem(rhs.inner))
	}
}

impl<T: Shl<u32, Output = T>, I> Shl<u32> for NumWrapper<T, I> {
	type Output = Self;

	fn shl(self, rhs: u32) -> Self {
		Self::from(self.inner.shl(rhs))
	}
}

impl<T: Shr<u32, Output = T>, I> Shr<u32> for NumWrapper<T, I> {
	type Output = Self;

	fn shr(self, rhs: u32) -> Self {
		Self::from(self.inner.shr(rhs))
	}
}

impl<T: AddAssign, I> AddAssign for NumWrapper<T, I> {
	fn add_assign(&mut self, rhs: Self) {
		self.inner.add_assign(rhs.inner)
	}
}

impl<T: SubAssign, I> SubAssign for NumWrapper<T, I> {
	fn sub_assign(&mut self, rhs: Self) {
		self.inner.sub_assign(rhs.inner)
	}
}

impl<T: MulAssign, I> MulAssign for NumWrapper<T, I> {
	fn mul_assign(&mut self, rhs: Self) {
		self.inner.mul_assign(rhs.inner)
	}
}

impl<T: DivAssign, I> DivAssign for NumWrapper<T, I> {
	fn div_assign(&mut self, rhs: Self) {
		self.inner.div_assign(rhs.inner)
	}
}

impl<T: RemAssign, I> RemAssign for NumWrapper<T, I> {
	fn rem_assign(&mut self, rhs: Self) {
		self.inner.rem_assign(rhs.inner)
	}
}

impl<T: CheckedAdd, I> CheckedAdd for NumWrapper<T, I> {
	fn checked_add(&self, rhs: &Self) -> Option<Self> {
		Some(Self::from(self.inner.checked_add(&rhs.inner)?))
	}
}

impl<T: CheckedSub, I> CheckedSub for NumWrapper<T, I> {
	fn checked_sub(&self, rhs: &Self) -> Option<Self> {
		Some(Self::from(self.inner.checked_sub(&rhs.inner)?))
	}
}

impl<T: CheckedMul, I> CheckedMul for NumWrapper<T, I> {
	fn checked_mul(&self, rhs: &Self) -> Option<Self> {
		Some(Self::from(self.inner.checked_mul(&rhs.inner)?))
	}
}

impl<T: CheckedDiv, I> CheckedDiv for NumWrapper<T, I> {
	fn checked_div(&self, rhs: &Self) -> Option<Self> {
		Some(Self::from(self.inner.checked_div(&rhs.inner)?))
	}
}

impl<T: CheckedRem, I> CheckedRem for NumWrapper<T, I> {
	fn checked_rem(&self, rhs: &Self) -> Option<Self> {
		Some(Self::from(self.inner.checked_rem(&rhs.inner)?))
	}
}

impl<T: CheckedShl, I> CheckedShl for NumWrapper<T, I> {
	fn checked_shl(&self, rhs: u32) -> Option<Self> {
		Some(Self::from(self.inner.checked_shl(rhs)?))
	}
}

impl<T: CheckedShr, I> CheckedShr for NumWrapper<T, I> {
	fn checked_shr(&self, rhs: u32) -> Option<Self> {
		Some(Self::from(self.inner.checked_shr(rhs)?))
	}
}

impl<T: CheckedNeg, I> CheckedNeg for NumWrapper<T, I> {
	fn checked_neg(&self) -> Option<Self> {
		Some(Self::from(self.inner.checked_neg()?))
	}
}

impl<T: Saturating, I> Saturating for NumWrapper<T, I> {
	fn saturating_add(self, rhs: Self) -> Self {
		Self::from(self.inner.saturating_add(rhs.inner))
	}

	fn saturating_sub(self, rhs: Self) -> Self {
		Self::from(self.inner.saturating_sub(rhs.inner))
	}

	fn saturating_mul(self, rhs: Self) -> Self {
		Self::from(self.inner.saturating_mul(rhs.inner))
	}

	fn saturating_pow(self, exp: usize) -> Self {
		Self::from(self.inner.saturating_pow(exp))
	}
}

impl<T: Zero, I> Zero for NumWrapper<T, I> {
	fn zero() -> Self {
		Self::from(T::zero())
	}

	fn is_zero(&self) -> bool {
		self.inner.is_zero()
	}
}

impl<T: One + PartialEq, I> One for NumWrapper<T, I> {
	fn one() -> Self {
		Self::from(T::one())
	}

	fn is_one(&self) -> bool {
		self.inner.is_one()
	}
}

impl<T: IntegerSquareRoot, I> IntegerSquareRoot for NumWrapper<T, I> {
	fn integer_sqrt_checked(&self) -> Option<Self> {
		Some(Self::from(self.inner.integer_sqrt_checked()?))
	}
}

impl<T: Bounded, I> Bounded for NumWrapper<T, I> {
	fn min_value() -> Self {
		Self::from(T::min_value())
	}

	fn max_value() -> Self {
		Self::from(T::max_value())
	}
}

/*
impl<T: Encode, I> Encode for NumWrapper<T, I> {
	fn encode(&self) -> Vec<u8> {
		self.inner.encode()
	}
}

impl<T: Decode, I> Decode for NumWrapper<T, I> {
	fn decode<In: Input>(input: &mut In) -> Result<Self, parity_scale_codec::Error> {
		Ok(Self::from(T::decode(input)?))
	}
}
*/

impl<T, I> Deref for NumWrapper<T, I> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T, I> WrapperTypeEncode for NumWrapper<T, I> {}

impl<T: Into<Self>, I> WrapperTypeDecode for NumWrapper<T, I> {
	type Wrapped = T;
}

impl<T, I> From<T> for NumWrapper<T, I> {
	fn from(other: T) -> Self {
		Self::from(other)
	}
}

impl<T, I> From<Compact<T>> for NumWrapper<T, I> {
	fn from(other: Compact<T>) -> Self {
		Self::from(other.0)
	}
}

impl<T, I> From<Compact<Self>> for NumWrapper<T, I> {
	fn from(other: Compact<Self>) -> Self {
		other.0
	}
}

impl<T: CompactAs, I> CompactAs for NumWrapper<T, I> {
	type As = T::As;

	fn encode_as(&self) -> &Self::As {
		self.inner.encode_as()
	}

	fn decode_from(x: Self::As) -> Result<Self, parity_scale_codec::Error> {
		Ok(Self::from(T::decode_from(x)?))
	}
}

impl<T: MaxEncodedLen, I> MaxEncodedLen for NumWrapper<T, I> {
	fn max_encoded_len() -> usize {
		T::max_encoded_len()
	}
}

impl<'a, T: 'a, I: 'a> EncodeAsRef<'a, Self> for NumWrapper<T, I>
where
	CompactRef<'a, T>: Encode + From<&'a Self>,
{
	type RefType = CompactRef<'a, T>;
}

impl<T: EncodeLike, I> EncodeLike for NumWrapper<T, I> {}
impl<T: Encode, I> EncodeLike<T> for NumWrapper<T, I> {}
impl<T: Encode, I> EncodeLike<T> for &NumWrapper<T, I> {}

#[cfg(test)]
mod tests {
	use sp_arithmetic::traits::BaseArithmetic;

	use super::*;

	/*
	#[derive(Debug, PartialEq, Encode, Decode)]
	enum TestGenericHasCompact {
		A {
			#[codec(compact)]
			a: NumWrapper<u64, ()>,
		},
	}
	*/

	fn is_has_compact<T: HasCompact>() {}

	#[test]
	fn ensure_is_has_compact() {
		/*
		#[derive(Encode, Decode, MaxEncodedLen)]
		struct Example {
			#[codec(compact)]
			a: NumWrapper<u64, ()>,
		}

		let example = Example { a: 256.into() };

		let encoded = (&example).encode();

		dbg!(encoded);
		panic!()
			*/

		is_has_compact::<NumWrapper<u64, ()>>();
	}

	/*
	#[test]
	fn foo() {
		Compact
	} EncodeAsRef,
	*/

	/*
	fn is_base_arithmetic<T: BaseArithmetic>() {}

	#[test]
	fn ensure_is_base_arithmetic() {
		is_base_arithmetic::<NumWrapper<u64, ()>>();
	}
	*/
}
