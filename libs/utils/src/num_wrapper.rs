use parity_scale_codec::{Compact, CompactAs, Decode, Encode, MaxEncodedLen};
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
		Add, AddAssign, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Shl, Shr, Sub, SubAssign,
	},
};

/// Type that allows to create different typed numbers with the same inner
/// type:
///
/// ```
/// # use cfg_utils::num_wrapper::NumWrapper;
///
/// struct Id1;
/// struct Id2;
///
/// type FooU64 = NumWrapper<u64, Id1>;
/// type BarU64 = NumWrapper<u64, Id2>;
/// ```
#[derive(TypeInfo, Serialize, Deserialize, Encode, Decode, MaxEncodedLen)]
#[scale_info(skip_type_params(T, I))]
pub struct NumWrapper<T, I> {
	inner: T,
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

macro_rules! const_methods {
	($t:ty) => {
		impl<I> NumWrapper<$t, I> {
			pub const MAX: Self = Self::from(<$t>::MAX);
			pub const MIN: Self = Self::from(<$t>::MIN);

			pub const fn add(self, other: Self) -> Self {
				Self::from(self.inner + other.inner)
			}

			pub const fn sub(self, other: Self) -> Self {
				Self::from(self.inner - other.inner)
			}

			pub const fn mul(self, other: Self) -> Self {
				Self::from(self.inner * other.inner)
			}

			pub const fn div(self, other: Self) -> Self {
				Self::from(self.inner / other.inner)
			}

			pub const fn saturating_add(self, other: Self) -> Self {
				Self::from(self.inner.saturating_add(other.inner))
			}

			pub const fn saturating_sub(self, other: Self) -> Self {
				Self::from(self.inner.saturating_sub(other.inner))
			}

			pub const fn saturating_mul(self, other: Self) -> Self {
				Self::from(self.inner.saturating_mul(other.inner))
			}

			pub const fn saturating_div(self, other: Self) -> Self {
				Self::from(self.inner.saturating_div(other.inner))
			}

			pub const fn add_int(self, other: $t) -> Self {
				Self::from(self.inner + other)
			}

			pub const fn sub_int(self, other: $t) -> Self {
				Self::from(self.inner - other)
			}

			pub const fn mul_int(self, other: $t) -> Self {
				Self::from(self.inner * other)
			}

			pub const fn div_int(self, other: $t) -> Self {
				Self::from(self.inner / other)
			}

			pub const fn get(self) -> $t {
				self.inner
			}
		}
	};
}

const_methods!(u8);
const_methods!(u16);
const_methods!(u32);
const_methods!(u64);
const_methods!(u128);

impl<T, I> From<T> for NumWrapper<T, I> {
	fn from(other: T) -> Self {
		Self::from((other).into())
	}
}

///  ------------------ From u8 -----------------------

impl<I> TryFrom<u16> for NumWrapper<u8, I> {
	type Error = <u8 as TryFrom<u16>>::Error;

	fn try_from(other: u16) -> Result<NumWrapper<u8, I>, Self::Error> {
		Ok(Self::from(u8::try_from(other)?))
	}
}

impl<I> TryFrom<u32> for NumWrapper<u8, I> {
	type Error = <u8 as TryFrom<u32>>::Error;

	fn try_from(other: u32) -> Result<NumWrapper<u8, I>, Self::Error> {
		Ok(Self::from(u8::try_from(other)?))
	}
}

impl<I> TryFrom<u64> for NumWrapper<u8, I> {
	type Error = <u8 as TryFrom<u64>>::Error;

	fn try_from(other: u64) -> Result<NumWrapper<u8, I>, Self::Error> {
		Ok(Self::from(u8::try_from(other)?))
	}
}

impl<I> TryFrom<u128> for NumWrapper<u8, I> {
	type Error = <u8 as TryFrom<u128>>::Error;

	fn try_from(other: u128) -> Result<NumWrapper<u8, I>, Self::Error> {
		Ok(Self::from(u8::try_from(other)?))
	}
}

impl<I> TryFrom<usize> for NumWrapper<u8, I> {
	type Error = <u8 as TryFrom<usize>>::Error;

	fn try_from(other: usize) -> Result<NumWrapper<u8, I>, Self::Error> {
		Ok(Self::from(u8::try_from(other)?))
	}
}

///  ------------------ From u16 -----------------------

impl<I> From<u8> for NumWrapper<u16, I> {
	fn from(other: u8) -> NumWrapper<u16, I> {
		Self::from(other as u16)
	}
}

impl<I> TryFrom<u32> for NumWrapper<u16, I> {
	type Error = <u16 as TryFrom<u32>>::Error;

	fn try_from(other: u32) -> Result<NumWrapper<u16, I>, Self::Error> {
		Ok(Self::from(u16::try_from(other)?))
	}
}

impl<I> TryFrom<u64> for NumWrapper<u16, I> {
	type Error = <u16 as TryFrom<u64>>::Error;

	fn try_from(other: u64) -> Result<NumWrapper<u16, I>, Self::Error> {
		Ok(Self::from(u16::try_from(other)?))
	}
}

impl<I> TryFrom<u128> for NumWrapper<u16, I> {
	type Error = <u16 as TryFrom<u128>>::Error;

	fn try_from(other: u128) -> Result<NumWrapper<u16, I>, Self::Error> {
		Ok(Self::from(u16::try_from(other)?))
	}
}

impl<I> TryFrom<usize> for NumWrapper<u16, I> {
	type Error = <u16 as TryFrom<usize>>::Error;

	fn try_from(other: usize) -> Result<NumWrapper<u16, I>, Self::Error> {
		Ok(Self::from(u16::try_from(other)?))
	}
}

///  ------------------ From u32 -----------------------

impl<I> From<u8> for NumWrapper<u32, I> {
	fn from(other: u8) -> NumWrapper<u32, I> {
		Self::from(other as u32)
	}
}

impl<I> From<u16> for NumWrapper<u32, I> {
	fn from(other: u16) -> NumWrapper<u32, I> {
		Self::from(other as u32)
	}
}

impl<I> TryFrom<u64> for NumWrapper<u32, I> {
	type Error = <u32 as TryFrom<u64>>::Error;

	fn try_from(other: u64) -> Result<NumWrapper<u32, I>, Self::Error> {
		Ok(Self::from(u32::try_from(other)?))
	}
}

impl<I> TryFrom<u128> for NumWrapper<u32, I> {
	type Error = <u32 as TryFrom<u128>>::Error;

	fn try_from(other: u128) -> Result<NumWrapper<u32, I>, Self::Error> {
		Ok(Self::from(u32::try_from(other)?))
	}
}

impl<I> TryFrom<usize> for NumWrapper<u32, I> {
	type Error = <u32 as TryFrom<usize>>::Error;

	fn try_from(other: usize) -> Result<NumWrapper<u32, I>, Self::Error> {
		Ok(Self::from(u32::try_from(other)?))
	}
}

///  ------------------ From u64 -----------------------

impl<I> From<u8> for NumWrapper<u64, I> {
	fn from(other: u8) -> NumWrapper<u64, I> {
		Self::from(other as u64)
	}
}

impl<I> From<u16> for NumWrapper<u64, I> {
	fn from(other: u16) -> NumWrapper<u64, I> {
		Self::from(other as u64)
	}
}

impl<I> From<u32> for NumWrapper<u64, I> {
	fn from(other: u32) -> NumWrapper<u64, I> {
		Self::from(other as u64)
	}
}

impl<I> TryFrom<u128> for NumWrapper<u64, I> {
	type Error = <u64 as TryFrom<u128>>::Error;

	fn try_from(other: u128) -> Result<NumWrapper<u64, I>, Self::Error> {
		Ok(Self::from(u64::try_from(other)?))
	}
}

impl<I> TryFrom<usize> for NumWrapper<u64, I> {
	type Error = <u64 as TryFrom<usize>>::Error;

	fn try_from(other: usize) -> Result<NumWrapper<u64, I>, Self::Error> {
		Ok(Self::from(u64::try_from(other)?))
	}
}

///  ------------------ From u128 -----------------------

impl<I> From<u8> for NumWrapper<u128, I> {
	fn from(other: u8) -> NumWrapper<u128, I> {
		Self::from(other as u128)
	}
}

impl<I> From<u16> for NumWrapper<u128, I> {
	fn from(other: u16) -> NumWrapper<u128, I> {
		Self::from(other as u128)
	}
}

impl<I> From<u32> for NumWrapper<u128, I> {
	fn from(other: u32) -> NumWrapper<u128, I> {
		Self::from(other as u128)
	}
}

impl<I> From<u64> for NumWrapper<u128, I> {
	fn from(other: u64) -> NumWrapper<u128, I> {
		Self::from(other as u128)
	}
}

impl<I> TryFrom<usize> for NumWrapper<u128, I> {
	type Error = <u128 as TryFrom<usize>>::Error;

	fn try_from(other: usize) -> Result<NumWrapper<u128, I>, Self::Error> {
		Ok(Self::from(u128::try_from(other)?))
	}
}

/// -------------------  Into T -----------------------

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

impl<T: Into<u128>, I> Into<u128> for NumWrapper<T, I> {
	fn into(self) -> u128 {
		self.inner.into()
	}
}

impl<T: TryInto<usize>, I> TryInto<usize> for NumWrapper<T, I> {
	type Error = T::Error;

	fn try_into(self) -> Result<usize, Self::Error> {
		Ok(self.inner.try_into()?)
	}
}

/// -----------------------------------------------------

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

impl<T: Add<Output = T>, I> Add<T> for NumWrapper<T, I> {
	type Output = Self;

	fn add(self, rhs: T) -> Self {
		Self::from(self.inner.add(rhs))
	}
}

impl<T: Sub<Output = T>, I> Sub<T> for NumWrapper<T, I> {
	type Output = Self;

	fn sub(self, rhs: T) -> Self {
		Self::from(self.inner.sub(rhs))
	}
}

impl<T: Mul<Output = T>, I> Mul<T> for NumWrapper<T, I> {
	type Output = Self;

	fn mul(self, rhs: T) -> Self {
		Self::from(self.inner.mul(rhs))
	}
}

impl<T: Div<Output = T>, I> Div<T> for NumWrapper<T, I> {
	type Output = Self;

	fn div(self, rhs: T) -> Self {
		Self::from(self.inner.div(rhs))
	}
}

impl<T: Rem<Output = T>, I> Rem<T> for NumWrapper<T, I> {
	type Output = Self;

	fn rem(self, rhs: T) -> Self {
		Self::from(self.inner.rem(rhs))
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

impl<T: AddAssign, I> AddAssign<T> for NumWrapper<T, I> {
	fn add_assign(&mut self, rhs: T) {
		self.inner.add_assign(rhs)
	}
}

impl<T: SubAssign, I> SubAssign<T> for NumWrapper<T, I> {
	fn sub_assign(&mut self, rhs: T) {
		self.inner.sub_assign(rhs)
	}
}

impl<T: MulAssign, I> MulAssign<T> for NumWrapper<T, I> {
	fn mul_assign(&mut self, rhs: T) {
		self.inner.mul_assign(rhs)
	}
}

impl<T: DivAssign, I> DivAssign<T> for NumWrapper<T, I> {
	fn div_assign(&mut self, rhs: T) {
		self.inner.div_assign(rhs)
	}
}

impl<T: RemAssign, I> RemAssign<T> for NumWrapper<T, I> {
	fn rem_assign(&mut self, rhs: T) {
		self.inner.rem_assign(rhs)
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

impl<T, I> From<Compact<Self>> for NumWrapper<T, I> {
	fn from(other: Compact<Self>) -> Self {
		other.0
	}
}

impl<T, I> CompactAs for NumWrapper<T, I> {
	type As = T;

	fn encode_as(&self) -> &Self::As {
		&self.inner
	}

	fn decode_from(x: Self::As) -> Result<Self, parity_scale_codec::Error> {
		Ok(Self::from(x))
	}
}

#[cfg(test)]
mod tests {
	use frame_support::Parameter;
	use parity_scale_codec::{EncodeLike, HasCompact};
	use sp_arithmetic::traits::BaseArithmetic;
	use sp_runtime::{traits::Member, FixedPointOperand};

	use super::*;

	fn is_has_compact<T: HasCompact>() {}
	fn is_base_arithmetic<T: BaseArithmetic>() {}
	fn is_encode<T: Encode + Decode + MaxEncodedLen>() {}
	fn is_member<T: Member>() {}
	fn is_parameter<T: Parameter>() {}
	fn is_type_info<T: TypeInfo>() {}
	fn is_encode_like<T: EncodeLike>() {}
	fn is_fixed_point_operand<T: FixedPointOperand>() {}

	// Id does not require any implementation
	struct Id;

	macro_rules! check_wrapper {
		($name:ident, $t:ty) => {
			mod $name {
				use super::*;

				#[test]
				fn check_wrapper() {
					type Num = NumWrapper<$t, Id>;

					is_has_compact::<Num>();
					is_base_arithmetic::<Num>();
					is_encode::<Num>();
					is_member::<Num>();
					is_parameter::<Num>();
					is_type_info::<Num>();
					is_encode_like::<Num>();
					is_fixed_point_operand::<Num>();
				}
			}
		};
	}

	check_wrapper!(u8_type, u8);
	check_wrapper!(u16_type, u8);
	check_wrapper!(u32_type, u8);
	check_wrapper!(u64_type, u8);
	check_wrapper!(u128_type, u8);
}
