#![feature(prelude_import)]
//! # Common types and primitives used for Centrifuge chain runtime.
#[prelude_import]
use std::prelude::rust_2018::*;
#[macro_use]
extern crate std;
pub use apis::*;
pub use constants::*;
pub use impls::*;
pub use types::*;
mod fixed_point {
    //! Decimal Fixed Point implementations for Substrate runtime.
    //! Copied over from sp_arithmetic
    use codec::{CompactAs, Decode, Encode};
    use sp_arithmetic::{
        helpers_128bit::multiply_by_rational,
        traits::{Bounded, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Saturating, Zero},
        FixedPointNumber, FixedPointOperand,
    };
    use sp_std::{
        convert::TryInto,
        ops::{self},
        prelude::*,
    };
    #[cfg(feature = "std")]
    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
    /// Data type used as intermediate storage in some computations to avoid overflow.
    struct I129 {
        value: u128,
        negative: bool,
    }
    impl<N: FixedPointOperand> From<N> for I129 {
        fn from(n: N) -> I129 {
            if n < N::zero() {
                let value: u128 = n
                    .checked_neg()
                    .map(|n| n.unique_saturated_into())
                    .unwrap_or_else(|| N::max_value().unique_saturated_into().saturating_add(1));
                I129 {
                    value,
                    negative: true,
                }
            } else {
                I129 {
                    value: n.unique_saturated_into(),
                    negative: false,
                }
            }
        }
    }
    /// Transforms an `I129` to `N` if it is possible.
    fn from_i129<N: FixedPointOperand>(n: I129) -> Option<N> {
        let max_plus_one: u128 = N::max_value().unique_saturated_into().saturating_add(1);
        if n.negative && N::min_value() < N::zero() && n.value == max_plus_one {
            Some(N::min_value())
        } else {
            let unsigned_inner: N = n.value.try_into().ok()?;
            let inner = if n.negative {
                unsigned_inner.checked_neg()?
            } else {
                unsigned_inner
            };
            Some(inner)
        }
    }
    /// Returns `R::max` if the sign of `n * m` is positive, `R::min` otherwise.
    fn to_bound<N: FixedPointOperand, D: FixedPointOperand, R: Bounded>(n: N, m: D) -> R {
        if (n < N::zero()) != (m < D::zero()) {
            R::min_value()
        } else {
            R::max_value()
        }
    }
    /// A fixed point number representation in the range.
    ///_Fixed Point 128 bits unsigned type as Amount, range = [0.000000000000000000, 340282366920938463463.374607431768211455]_
    pub struct Amount(u128);
    const _: () = {
        impl ::codec::Encode for Amount {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.0, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.0)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.0, f)
            }
        }
        impl ::codec::EncodeLike for Amount {}
    };
    const _: () = {
        impl ::codec::Decode for Amount {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(Amount({
                    let __codec_res_edqy = <u128 as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `Amount.0`"),
                            )
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                }))
            }
        }
    };
    const _: () = {
        impl ::codec::CompactAs for Amount {
            type As = u128;
            fn encode_as(&self) -> &u128 {
                &self.0
            }
            fn decode_from(x: u128) -> ::core::result::Result<Amount, ::codec::Error> {
                ::core::result::Result::Ok(Amount(x))
            }
        }
        impl From<::codec::Compact<Amount>> for Amount {
            fn from(x: ::codec::Compact<Amount>) -> Amount {
                x.0
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for Amount {
        #[inline]
        fn default() -> Amount {
            Amount(::core::default::Default::default())
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for Amount {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for Amount {
        #[inline]
        fn clone(&self) -> Amount {
            {
                let _: ::core::clone::AssertParamIsClone<u128>;
                *self
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for Amount {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for Amount {
        #[inline]
        fn eq(&self, other: &Amount) -> bool {
            match *other {
                Amount(ref __self_1_0) => match *self {
                    Amount(ref __self_0_0) => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &Amount) -> bool {
            match *other {
                Amount(ref __self_1_0) => match *self {
                    Amount(ref __self_0_0) => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for Amount {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for Amount {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<u128>;
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialOrd for Amount {
        #[inline]
        fn partial_cmp(&self, other: &Amount) -> ::core::option::Option<::core::cmp::Ordering> {
            match *other {
                Amount(ref __self_1_0) => match *self {
                    Amount(ref __self_0_0) => {
                        match ::core::cmp::PartialOrd::partial_cmp(&(*__self_0_0), &(*__self_1_0)) {
                            ::core::option::Option::Some(::core::cmp::Ordering::Equal) => {
                                ::core::option::Option::Some(::core::cmp::Ordering::Equal)
                            }
                            cmp => cmp,
                        }
                    }
                },
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Ord for Amount {
        #[inline]
        fn cmp(&self, other: &Amount) -> ::core::cmp::Ordering {
            match *other {
                Amount(ref __self_1_0) => match *self {
                    Amount(ref __self_0_0) => {
                        match ::core::cmp::Ord::cmp(&(*__self_0_0), &(*__self_1_0)) {
                            ::core::cmp::Ordering::Equal => ::core::cmp::Ordering::Equal,
                            cmp => cmp,
                        }
                    }
                },
            }
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for Amount {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                :: scale_info :: Type :: builder () . path (:: scale_info :: Path :: new ("Amount" , "runtime_common::fixed_point")) . type_params (:: alloc :: vec :: Vec :: new ()) . docs (& ["A fixed point number representation in the range." , "_Fixed Point 128 bits unsigned type as Amount, range = [0.000000000000000000, 340282366920938463463.374607431768211455]_"]) . composite (:: scale_info :: build :: Fields :: unnamed () . field (| f | f . ty :: < u128 > () . type_name ("u128") . docs (& [])))
            }
        };
    };
    impl From<u128> for Amount {
        fn from(int: u128) -> Self {
            Amount::saturating_from_integer(int)
        }
    }
    impl<N: FixedPointOperand, D: FixedPointOperand> From<(N, D)> for Amount {
        fn from(r: (N, D)) -> Self {
            Amount::saturating_from_rational(r.0, r.1)
        }
    }
    impl FixedPointNumber for Amount {
        type Inner = u128;
        const DIV: Self::Inner = 1_000_000_000_000_000_000;
        const SIGNED: bool = false;
        fn from_inner(inner: Self::Inner) -> Self {
            Self(inner)
        }
        fn into_inner(self) -> Self::Inner {
            self.0
        }
    }
    impl Amount {
        /// const version of `FixedPointNumber::from_inner`.
        pub const fn from_inner(inner: u128) -> Self {
            Self(inner)
        }
        #[cfg(any(feature = "std", test))]
        pub fn from_float(x: f64) -> Self {
            Self((x * (<Self as FixedPointNumber>::DIV as f64)) as u128)
        }
        #[cfg(any(feature = "std", test))]
        pub fn to_float(self) -> f64 {
            self.0 as f64 / <Self as FixedPointNumber>::DIV as f64
        }
    }
    impl Saturating for Amount {
        fn saturating_add(self, rhs: Self) -> Self {
            Self(self.0.saturating_add(rhs.0))
        }
        fn saturating_sub(self, rhs: Self) -> Self {
            Self(self.0.saturating_sub(rhs.0))
        }
        fn saturating_mul(self, rhs: Self) -> Self {
            self.checked_mul(&rhs)
                .unwrap_or_else(|| to_bound(self.0, rhs.0))
        }
        fn saturating_pow(self, exp: usize) -> Self {
            if exp == 0 {
                return Self::saturating_from_integer(1);
            }
            let exp = exp as u32;
            let msb_pos = 32 - exp.leading_zeros();
            let mut result = Self::saturating_from_integer(1);
            let mut pow_val = self;
            for i in 0..msb_pos {
                if ((1 << i) & exp) > 0 {
                    result = result.saturating_mul(pow_val);
                }
                pow_val = pow_val.saturating_mul(pow_val);
            }
            result
        }
    }
    impl ops::Neg for Amount {
        type Output = Self;
        fn neg(self) -> Self::Output {
            Self(<Self as FixedPointNumber>::Inner::zero() - self.0)
        }
    }
    impl ops::Add for Amount {
        type Output = Self;
        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0 + rhs.0)
        }
    }
    impl ops::Sub for Amount {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self::Output {
            Self(self.0 - rhs.0)
        }
    }
    impl ops::Mul for Amount {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self::Output {
            self.checked_mul(&rhs)
                .unwrap_or_else(|| ::std::rt::begin_panic("attempt to multiply with overflow"))
        }
    }
    impl ops::Div for Amount {
        type Output = Self;
        fn div(self, rhs: Self) -> Self::Output {
            if rhs.0 == 0 {
                {
                    ::std::rt::begin_panic("attempt to divide by zero")
                }
            }
            self.checked_div(&rhs)
                .unwrap_or_else(|| ::std::rt::begin_panic("attempt to divide with overflow"))
        }
    }
    impl CheckedSub for Amount {
        fn checked_sub(&self, rhs: &Self) -> Option<Self> {
            self.0.checked_sub(rhs.0).map(Self)
        }
    }
    impl CheckedAdd for Amount {
        fn checked_add(&self, rhs: &Self) -> Option<Self> {
            self.0.checked_add(rhs.0).map(Self)
        }
    }
    impl CheckedDiv for Amount {
        fn checked_div(&self, other: &Self) -> Option<Self> {
            if other.0 == 0 {
                return None;
            }
            let lhs: I129 = self.0.into();
            let rhs: I129 = other.0.into();
            let negative = lhs.negative != rhs.negative;
            multiply_by_rational(lhs.value, Self::DIV as u128, rhs.value)
                .ok()
                .and_then(|value| from_i129(I129 { value, negative }))
                .map(Self)
        }
    }
    impl CheckedMul for Amount {
        fn checked_mul(&self, other: &Self) -> Option<Self> {
            let lhs: I129 = self.0.into();
            let rhs: I129 = other.0.into();
            let negative = lhs.negative != rhs.negative;
            multiply_by_rational(lhs.value, rhs.value, Self::DIV as u128)
                .ok()
                .and_then(|value| from_i129(I129 { value, negative }))
                .map(Self)
        }
    }
    impl Bounded for Amount {
        fn min_value() -> Self {
            Self(<Self as FixedPointNumber>::Inner::min_value())
        }
        fn max_value() -> Self {
            Self(<Self as FixedPointNumber>::Inner::max_value())
        }
    }
    impl Zero for Amount {
        fn zero() -> Self {
            Self::from_inner(<Self as FixedPointNumber>::Inner::zero())
        }
        fn is_zero(&self) -> bool {
            self.into_inner() == <Self as FixedPointNumber>::Inner::zero()
        }
    }
    impl One for Amount {
        fn one() -> Self {
            Self::from_inner(Self::DIV)
        }
    }
    impl sp_std::fmt::Debug for Amount {
        #[cfg(feature = "std")]
        fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
            let integral = {
                let int = self.0 / Self::accuracy();
                let signum_for_zero = if int == 0 && self.is_negative() {
                    "-"
                } else {
                    ""
                };
                {
                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                        &["", ""],
                        &match (&signum_for_zero, &int) {
                            _args => [
                                ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                                ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                            ],
                        },
                    ));
                    res
                }
            };
            let precision = (Self::accuracy() as f64).log10() as usize;
            let fractional = {
                let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1_formatted(
                    &[""],
                    &match (&((self.0 % Self::accuracy()) as i128).abs(), &precision) {
                        _args => [
                            ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                            ::core::fmt::ArgumentV1::from_usize(_args.1),
                        ],
                    },
                    &[::core::fmt::rt::v1::Argument {
                        position: 0usize,
                        format: ::core::fmt::rt::v1::FormatSpec {
                            fill: '0',
                            align: ::core::fmt::rt::v1::Alignment::Right,
                            flags: 0u32,
                            precision: ::core::fmt::rt::v1::Count::Implied,
                            width: ::core::fmt::rt::v1::Count::Param(1usize),
                        },
                    }],
                    unsafe { ::core::fmt::UnsafeArg::new() },
                ));
                res
            };
            f.write_fmt(::core::fmt::Arguments::new_v1(
                &["", "(", ".", ")"],
                &match (&"Amount", &integral, &fractional) {
                    _args => [
                        ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                        ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                        ::core::fmt::ArgumentV1::new(_args.2, ::core::fmt::Display::fmt),
                    ],
                },
            ))
        }
    }
    #[cfg(feature = "std")]
    impl sp_std::fmt::Display for Amount {
        fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
            f.write_fmt(::core::fmt::Arguments::new_v1(
                &[""],
                &match (&self.0,) {
                    _args => [::core::fmt::ArgumentV1::new(
                        _args.0,
                        ::core::fmt::Display::fmt,
                    )],
                },
            ))
        }
    }
    #[cfg(feature = "std")]
    impl sp_std::str::FromStr for Amount {
        type Err = &'static str;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let inner: <Self as FixedPointNumber>::Inner = s
                .parse()
                .map_err(|_| "invalid string input for fixed point number")?;
            Ok(Self::from_inner(inner))
        }
    }
    #[cfg(feature = "std")]
    impl Serialize for Amount {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.to_string())
        }
    }
    #[cfg(feature = "std")]
    impl<'de> Deserialize<'de> for Amount {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            use sp_std::str::FromStr;
            let s = String::deserialize(deserializer)?;
            Amount::from_str(&s).map_err(de::Error::custom)
        }
    }
    impl From<Amount> for u128 {
        fn from(amount: Amount) -> Self {
            amount.into_inner()
        }
    }
    /// A fixed point number representation in the range.
    ///_Fixed Point 128 bits unsigned with 27 precision for Rate
    pub struct Rate(u128);
    const _: () = {
        impl ::codec::Encode for Rate {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.0, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.0)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.0, f)
            }
        }
        impl ::codec::EncodeLike for Rate {}
    };
    const _: () = {
        impl ::codec::Decode for Rate {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(Rate({
                    let __codec_res_edqy = <u128 as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `Rate.0`"),
                            )
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                }))
            }
        }
    };
    const _: () = {
        impl ::codec::CompactAs for Rate {
            type As = u128;
            fn encode_as(&self) -> &u128 {
                &self.0
            }
            fn decode_from(x: u128) -> ::core::result::Result<Rate, ::codec::Error> {
                ::core::result::Result::Ok(Rate(x))
            }
        }
        impl From<::codec::Compact<Rate>> for Rate {
            fn from(x: ::codec::Compact<Rate>) -> Rate {
                x.0
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for Rate {
        #[inline]
        fn default() -> Rate {
            Rate(::core::default::Default::default())
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for Rate {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for Rate {
        #[inline]
        fn clone(&self) -> Rate {
            {
                let _: ::core::clone::AssertParamIsClone<u128>;
                *self
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for Rate {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for Rate {
        #[inline]
        fn eq(&self, other: &Rate) -> bool {
            match *other {
                Rate(ref __self_1_0) => match *self {
                    Rate(ref __self_0_0) => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &Rate) -> bool {
            match *other {
                Rate(ref __self_1_0) => match *self {
                    Rate(ref __self_0_0) => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for Rate {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for Rate {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<u128>;
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialOrd for Rate {
        #[inline]
        fn partial_cmp(&self, other: &Rate) -> ::core::option::Option<::core::cmp::Ordering> {
            match *other {
                Rate(ref __self_1_0) => match *self {
                    Rate(ref __self_0_0) => {
                        match ::core::cmp::PartialOrd::partial_cmp(&(*__self_0_0), &(*__self_1_0)) {
                            ::core::option::Option::Some(::core::cmp::Ordering::Equal) => {
                                ::core::option::Option::Some(::core::cmp::Ordering::Equal)
                            }
                            cmp => cmp,
                        }
                    }
                },
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Ord for Rate {
        #[inline]
        fn cmp(&self, other: &Rate) -> ::core::cmp::Ordering {
            match *other {
                Rate(ref __self_1_0) => match *self {
                    Rate(ref __self_0_0) => {
                        match ::core::cmp::Ord::cmp(&(*__self_0_0), &(*__self_1_0)) {
                            ::core::cmp::Ordering::Equal => ::core::cmp::Ordering::Equal,
                            cmp => cmp,
                        }
                    }
                },
            }
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for Rate {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "Rate",
                        "runtime_common::fixed_point",
                    ))
                    .type_params(::alloc::vec::Vec::new())
                    .docs(&[
                        "A fixed point number representation in the range.",
                        "_Fixed Point 128 bits unsigned with 27 precision for Rate",
                    ])
                    .composite(
                        ::scale_info::build::Fields::unnamed()
                            .field(|f| f.ty::<u128>().type_name("u128").docs(&[])),
                    )
            }
        };
    };
    impl From<u128> for Rate {
        fn from(int: u128) -> Self {
            Rate::saturating_from_integer(int)
        }
    }
    impl<N: FixedPointOperand, D: FixedPointOperand> From<(N, D)> for Rate {
        fn from(r: (N, D)) -> Self {
            Rate::saturating_from_rational(r.0, r.1)
        }
    }
    impl FixedPointNumber for Rate {
        type Inner = u128;
        const DIV: Self::Inner = 1_000_000_000_000_000_000_000_000_000;
        const SIGNED: bool = false;
        fn from_inner(inner: Self::Inner) -> Self {
            Self(inner)
        }
        fn into_inner(self) -> Self::Inner {
            self.0
        }
    }
    impl Rate {
        /// const version of `FixedPointNumber::from_inner`.
        pub const fn from_inner(inner: u128) -> Self {
            Self(inner)
        }
        #[cfg(any(feature = "std", test))]
        pub fn from_float(x: f64) -> Self {
            Self((x * (<Self as FixedPointNumber>::DIV as f64)) as u128)
        }
        #[cfg(any(feature = "std", test))]
        pub fn to_float(self) -> f64 {
            self.0 as f64 / <Self as FixedPointNumber>::DIV as f64
        }
    }
    impl Saturating for Rate {
        fn saturating_add(self, rhs: Self) -> Self {
            Self(self.0.saturating_add(rhs.0))
        }
        fn saturating_sub(self, rhs: Self) -> Self {
            Self(self.0.saturating_sub(rhs.0))
        }
        fn saturating_mul(self, rhs: Self) -> Self {
            self.checked_mul(&rhs)
                .unwrap_or_else(|| to_bound(self.0, rhs.0))
        }
        fn saturating_pow(self, exp: usize) -> Self {
            if exp == 0 {
                return Self::saturating_from_integer(1);
            }
            let exp = exp as u32;
            let msb_pos = 32 - exp.leading_zeros();
            let mut result = Self::saturating_from_integer(1);
            let mut pow_val = self;
            for i in 0..msb_pos {
                if ((1 << i) & exp) > 0 {
                    result = result.saturating_mul(pow_val);
                }
                pow_val = pow_val.saturating_mul(pow_val);
            }
            result
        }
    }
    impl ops::Neg for Rate {
        type Output = Self;
        fn neg(self) -> Self::Output {
            Self(<Self as FixedPointNumber>::Inner::zero() - self.0)
        }
    }
    impl ops::Add for Rate {
        type Output = Self;
        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0 + rhs.0)
        }
    }
    impl ops::Sub for Rate {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self::Output {
            Self(self.0 - rhs.0)
        }
    }
    impl ops::Mul for Rate {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self::Output {
            self.checked_mul(&rhs)
                .unwrap_or_else(|| ::std::rt::begin_panic("attempt to multiply with overflow"))
        }
    }
    impl ops::Div for Rate {
        type Output = Self;
        fn div(self, rhs: Self) -> Self::Output {
            if rhs.0 == 0 {
                {
                    ::std::rt::begin_panic("attempt to divide by zero")
                }
            }
            self.checked_div(&rhs)
                .unwrap_or_else(|| ::std::rt::begin_panic("attempt to divide with overflow"))
        }
    }
    impl CheckedSub for Rate {
        fn checked_sub(&self, rhs: &Self) -> Option<Self> {
            self.0.checked_sub(rhs.0).map(Self)
        }
    }
    impl CheckedAdd for Rate {
        fn checked_add(&self, rhs: &Self) -> Option<Self> {
            self.0.checked_add(rhs.0).map(Self)
        }
    }
    impl CheckedDiv for Rate {
        fn checked_div(&self, other: &Self) -> Option<Self> {
            if other.0 == 0 {
                return None;
            }
            let lhs: I129 = self.0.into();
            let rhs: I129 = other.0.into();
            let negative = lhs.negative != rhs.negative;
            multiply_by_rational(lhs.value, Self::DIV as u128, rhs.value)
                .ok()
                .and_then(|value| from_i129(I129 { value, negative }))
                .map(Self)
        }
    }
    impl CheckedMul for Rate {
        fn checked_mul(&self, other: &Self) -> Option<Self> {
            let lhs: I129 = self.0.into();
            let rhs: I129 = other.0.into();
            let negative = lhs.negative != rhs.negative;
            multiply_by_rational(lhs.value, rhs.value, Self::DIV as u128)
                .ok()
                .and_then(|value| from_i129(I129 { value, negative }))
                .map(Self)
        }
    }
    impl Bounded for Rate {
        fn min_value() -> Self {
            Self(<Self as FixedPointNumber>::Inner::min_value())
        }
        fn max_value() -> Self {
            Self(<Self as FixedPointNumber>::Inner::max_value())
        }
    }
    impl Zero for Rate {
        fn zero() -> Self {
            Self::from_inner(<Self as FixedPointNumber>::Inner::zero())
        }
        fn is_zero(&self) -> bool {
            self.into_inner() == <Self as FixedPointNumber>::Inner::zero()
        }
    }
    impl One for Rate {
        fn one() -> Self {
            Self::from_inner(Self::DIV)
        }
    }
    impl sp_std::fmt::Debug for Rate {
        #[cfg(feature = "std")]
        fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
            let integral = {
                let int = self.0 / Self::accuracy();
                let signum_for_zero = if int == 0 && self.is_negative() {
                    "-"
                } else {
                    ""
                };
                {
                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                        &["", ""],
                        &match (&signum_for_zero, &int) {
                            _args => [
                                ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                                ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                            ],
                        },
                    ));
                    res
                }
            };
            let precision = (Self::accuracy() as f64).log10() as usize;
            let fractional = {
                let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1_formatted(
                    &[""],
                    &match (&((self.0 % Self::accuracy()) as i128).abs(), &precision) {
                        _args => [
                            ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                            ::core::fmt::ArgumentV1::from_usize(_args.1),
                        ],
                    },
                    &[::core::fmt::rt::v1::Argument {
                        position: 0usize,
                        format: ::core::fmt::rt::v1::FormatSpec {
                            fill: '0',
                            align: ::core::fmt::rt::v1::Alignment::Right,
                            flags: 0u32,
                            precision: ::core::fmt::rt::v1::Count::Implied,
                            width: ::core::fmt::rt::v1::Count::Param(1usize),
                        },
                    }],
                    unsafe { ::core::fmt::UnsafeArg::new() },
                ));
                res
            };
            f.write_fmt(::core::fmt::Arguments::new_v1(
                &["", "(", ".", ")"],
                &match (&"Rate", &integral, &fractional) {
                    _args => [
                        ::core::fmt::ArgumentV1::new(_args.0, ::core::fmt::Display::fmt),
                        ::core::fmt::ArgumentV1::new(_args.1, ::core::fmt::Display::fmt),
                        ::core::fmt::ArgumentV1::new(_args.2, ::core::fmt::Display::fmt),
                    ],
                },
            ))
        }
    }
    #[cfg(feature = "std")]
    impl sp_std::fmt::Display for Rate {
        fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
            f.write_fmt(::core::fmt::Arguments::new_v1(
                &[""],
                &match (&self.0,) {
                    _args => [::core::fmt::ArgumentV1::new(
                        _args.0,
                        ::core::fmt::Display::fmt,
                    )],
                },
            ))
        }
    }
    #[cfg(feature = "std")]
    impl sp_std::str::FromStr for Rate {
        type Err = &'static str;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let inner: <Self as FixedPointNumber>::Inner = s
                .parse()
                .map_err(|_| "invalid string input for fixed point number")?;
            Ok(Self::from_inner(inner))
        }
    }
    #[cfg(feature = "std")]
    impl Serialize for Rate {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.to_string())
        }
    }
    #[cfg(feature = "std")]
    impl<'de> Deserialize<'de> for Rate {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            use sp_std::str::FromStr;
            let s = String::deserialize(deserializer)?;
            Rate::from_str(&s).map_err(de::Error::custom)
        }
    }
}
mod impls {
    //! Some configurable implementations as associated type for the substrate runtime.
    use super::*;
    use codec::{Decode, Encode};
    use core::marker::PhantomData;
    use frame_support::sp_runtime::app_crypto::sp_core::U256;
    use frame_support::traits::{Currency, OnUnbalanced};
    use frame_support::weights::{
        WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
    };
    use frame_system::pallet::Config as SystemConfig;
    use pallet_authorship::{Config as AuthorshipConfig, Pallet as Authorship};
    use pallet_balances::{Config as BalancesConfig, Pallet as Balances};
    use pallet_permissions::Properties;
    use pallet_tinlake_investor_pool::Config;
    use primitives_tokens::CurrencyId;
    use scale_info::TypeInfo;
    use smallvec::smallvec;
    use sp_arithmetic::Perbill;
    use sp_core::H160;
    use sp_std::convert::TryInto;
    use sp_std::vec;
    use sp_std::vec::Vec;
    pub struct TrancheToken<T>(core::marker::PhantomData<T>);
    impl<T> pallet_tinlake_investor_pool::TrancheToken<T> for TrancheToken<T>
    where
        T: Config,
        <T as Config>::PoolId: Into<u64>,
        <T as Config>::TrancheId: Into<u8>,
        <T as Config>::CurrencyId: From<CurrencyId>,
    {
        fn tranche_token(
            pool: <T as Config>::PoolId,
            tranche: <T as Config>::TrancheId,
        ) -> <T as Config>::CurrencyId {
            CurrencyId::Tranche(pool.into(), tranche.into()).into()
        }
    }
    pub struct DealWithFees<Config>(PhantomData<Config>);
    pub type NegativeImbalance<Config> =
        <Balances<Config> as Currency<<Config as SystemConfig>::AccountId>>::NegativeImbalance;
    impl<Config> OnUnbalanced<NegativeImbalance<Config>> for DealWithFees<Config>
    where
        Config: AuthorshipConfig + BalancesConfig + SystemConfig,
    {
        fn on_nonzero_unbalanced(amount: NegativeImbalance<Config>) {
            Balances::<Config>::resolve_creating(&Authorship::<Config>::author(), amount);
        }
    }
    /// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
    /// node's balance type.
    ///
    /// This should typically create a mapping between the following ranges:
    ///   - [0, frame_system::MaximumBlockWeight]
    ///   - [Balance::min, Balance::max]
    ///
    /// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
    ///   - Setting it to `0` will essentially disable the weight fee.
    ///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
    ///
    /// Sample weight to Fee Calculation for 1 Rad Balance transfer:
    /// ```rust
    /// 	use node_primitives::Balance;
    /// 	let extrinsic_bytes: Balance = 92;
    /// 	let weight: Balance = 195000000;
    /// 	let weight_coefficient: Balance = 315000;
    /// 	let transaction_byte_fee: Balance = 10000000000; // 0.01 Micro RAD
    ///		let maximum_block_weight: Balance = 2000000000000; // 2 * WEIGHT_PER_SECOND
    /// 	let extrinsic_base_weight: Balance = 125000000; // 125 * WEIGHT_PER_MICROS
    ///
    /// 	// AIR token value
    ///     //
    ///     // FIXME (ToZ):
    ///     // The following constants are copied verbatim from Altair runtime constants so
    ///     // that to avoid a circular dependency between common runtime crate and Altair
    ///     // runtime crate. Can we consider such token values as primitives much like
    ///     // MILLISECONDS_PER_DAY constants, for instance, and extract them in a separate
    ///     // library.
    /// 	let MICRO_AIR: Balance = runtime_common::constants::MICRO_CFG;
    /// 	let MILLI_AIR: Balance = runtime_common::constants::MILLI_CFG;
    /// 	let CENTI_AIR: Balance = runtime_common::constants::CENTI_CFG;
    /// 	let AIR: Balance = runtime_common::constants::CFG;
    ///
    /// 	// Calculation:
    /// 	let base_fee: Balance = extrinsic_base_weight * weight_coefficient; // 39375000000000
    /// 	let length_fee: Balance = extrinsic_bytes * transaction_byte_fee; // 920000000000
    /// 	let weight_fee: Balance = weight * weight_coefficient; // 61425000000000
    /// 	let fee: Balance = base_fee + length_fee + weight_fee;
    /// 	assert_eq!(fee, 10172 * (MICRO_AIR / 100));
    /// ```
    pub struct WeightToFee;
    impl WeightToFeePolynomial for WeightToFee {
        type Balance = Balance;
        fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
            {
                let count = 0usize + 1usize;
                #[allow(unused_mut)]
                let mut vec = ::smallvec::SmallVec::new();
                if count <= vec.inline_size() {
                    vec.push(WeightToFeeCoefficient {
                        coeff_integer: 315000,
                        coeff_frac: Perbill::zero(),
                        negative: false,
                        degree: 1,
                    });
                    vec
                } else {
                    ::smallvec::SmallVec::from_vec(<[_]>::into_vec(box [WeightToFeeCoefficient {
                        coeff_integer: 315000,
                        coeff_frac: Perbill::zero(),
                        negative: false,
                        degree: 1,
                    }]))
                }
            }
        }
    }
    /// All data for an instance of an NFT.
    pub struct AssetInfo {
        pub metadata: Bytes,
    }
    const _: () = {
        impl ::codec::Encode for AssetInfo {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.metadata, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.metadata)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.metadata, f)
            }
        }
        impl ::codec::EncodeLike for AssetInfo {}
    };
    const _: () = {
        impl ::codec::Decode for AssetInfo {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(AssetInfo {
                    metadata: {
                        let __codec_res_edqy =
                            <Bytes as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `AssetInfo::metadata`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                })
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for AssetInfo {
        #[inline]
        fn clone(&self) -> AssetInfo {
            match *self {
                AssetInfo {
                    metadata: ref __self_0_0,
                } => AssetInfo {
                    metadata: ::core::clone::Clone::clone(&(*__self_0_0)),
                },
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for AssetInfo {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for AssetInfo {
        #[inline]
        fn eq(&self, other: &AssetInfo) -> bool {
            match *other {
                AssetInfo {
                    metadata: ref __self_1_0,
                } => match *self {
                    AssetInfo {
                        metadata: ref __self_0_0,
                    } => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &AssetInfo) -> bool {
            match *other {
                AssetInfo {
                    metadata: ref __self_1_0,
                } => match *self {
                    AssetInfo {
                        metadata: ref __self_0_0,
                    } => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for AssetInfo {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for AssetInfo {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<Bytes>;
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for AssetInfo {
        #[inline]
        fn default() -> AssetInfo {
            AssetInfo {
                metadata: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for AssetInfo {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                AssetInfo {
                    metadata: ref __self_0_0,
                } => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_struct(f, "AssetInfo");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "metadata",
                        &&(*__self_0_0),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for AssetInfo {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "AssetInfo",
                        "runtime_common::impls",
                    ))
                    .type_params(::alloc::vec::Vec::new())
                    .docs(&["All data for an instance of an NFT."])
                    .composite(::scale_info::build::Fields::named().field(|f| {
                        f.ty::<Bytes>()
                            .name("metadata")
                            .type_name("Bytes")
                            .docs(&[])
                    }))
            }
        };
    };
    impl From<Bytes32> for EthAddress {
        fn from(v: Bytes32) -> Self {
            EthAddress(v[..32].try_into().expect("Address wraps a 32 byte array"))
        }
    }
    impl From<EthAddress> for Bytes32 {
        fn from(a: EthAddress) -> Self {
            a.0
        }
    }
    impl From<RegistryId> for EthAddress {
        fn from(r: RegistryId) -> Self {
            let padded = r . 0 . to_fixed_bytes () . iter () . copied () . chain ([0 ; 12] . iter () . copied ()) . collect :: < Vec < u8 > > () [.. 32] . try_into () . expect ("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail") ;
            EthAddress(padded)
        }
    }
    impl From<EthAddress> for RegistryId {
        fn from(a: EthAddress) -> Self {
            RegistryId(H160::from_slice(&a.0[..20]))
        }
    }
    impl From<[u8; 20]> for RegistryId {
        fn from(d: [u8; 20]) -> Self {
            RegistryId(H160::from(d))
        }
    }
    impl AsRef<[u8]> for RegistryId {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }
    impl common_traits::BigEndian<Vec<u8>> for TokenId {
        fn to_big_endian(&self) -> Vec<u8> {
            let mut data = ::alloc::vec::from_elem(0, 32);
            self.0.to_big_endian(&mut data);
            data
        }
    }
    impl From<U256> for TokenId {
        fn from(v: U256) -> Self {
            Self(v)
        }
    }
    impl From<u16> for InstanceId {
        fn from(v: u16) -> Self {
            Self(v as u128)
        }
    }
    impl From<u128> for InstanceId {
        fn from(v: u128) -> Self {
            Self(v)
        }
    }
    impl Properties for PoolRoles {
        type Property = Self;
        type Element = u32;
        fn exists(element: &Self::Element, property: Self::Property) -> bool {
            ::core::panicking::panic("not yet implemented")
        }
        fn rm(element: &mut Self::Element, property: Self::Property) {
            ::core::panicking::panic("not yet implemented")
        }
        fn add(element: &mut Self::Element, property: Self::Property) {
            ::core::panicking::panic("not yet implemented")
        }
    }
}
pub mod apis {
    use node_primitives::{BlockNumber, Hash};
    use pallet_anchors::AnchorData;
    use sp_api::decl_runtime_apis;
    #[doc(hidden)]
    mod sp_api_hidden_includes_DECL_RUNTIME_APIS {
        pub extern crate sp_api as sp_api;
    }
    #[doc(hidden)]
    #[allow(dead_code)]
    #[allow(deprecated)]
    pub mod runtime_decl_for_AnchorApi {
        use super::*;
        /// The API to query anchoring info.
        pub trait AnchorApi<Block: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockT> {
            fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
        }
        pub const VERSION: u32 = 1u32;
        pub const ID: [u8; 8] = [201u8, 55u8, 215u8, 156u8, 146u8, 196u8, 232u8, 114u8];
        #[cfg(any(feature = "std", test))]
        fn convert_between_block_types<
            I: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::Encode,
            R: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::Decode,
            F: FnOnce(
                self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::codec::Error,
            ) -> self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError,
        >(
            input: &I,
            map_error: F,
        ) -> std::result::Result<R, self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError>
        {
            < R as self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: DecodeLimit > :: decode_with_depth_limit (self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: MAX_EXTRINSIC_DEPTH , & self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: Encode :: encode (input) [..]) . map_err (map_error)
        }
        #[cfg(any(feature = "std", test))]
        pub fn get_anchor_by_id_native_call_generator<
            'a,
            ApiImpl: AnchorApi<Block>,
            NodeBlock: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockT,
            Block: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockT + 'a,
        >(
            id: Hash,
        ) -> impl FnOnce() -> std::result::Result<
            Option<AnchorData<Hash, BlockNumber>>,
            self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError,
        > + 'a {
            move || {
                let res = ApiImpl::get_anchor_by_id(id);
                Ok(res)
            }
        }
        #[cfg(any(feature = "std", test))]
        #[allow(clippy::too_many_arguments)]
        pub fn get_anchor_by_id_call_api_at<
            R: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::Encode
                + self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::Decode
                + PartialEq,
            NC: FnOnce() -> std::result::Result<
                    R,
                    self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError,
                > + std::panic::UnwindSafe,
            Block: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockT,
            T: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::CallApiAt<Block>,
        >(
            call_runtime_at: &T,
            at: &self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockId<Block>,
            args: Vec<u8>,
            changes: &std::cell::RefCell<
                self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::OverlayedChanges,
            >,
            storage_transaction_cache: &std::cell::RefCell<
                self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::StorageTransactionCache<
                    Block,
                    T::StateBackend,
                >,
            >,
            native_call: Option<NC>,
            context: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ExecutionContext,
            recorder: &Option<
                self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ProofRecorder<Block>,
            >,
        ) -> std::result::Result<
            self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::NativeOrEncoded<R>,
            self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError,
        > {
            let version = call_runtime_at.runtime_version_at(at)?;
            let params = self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::CallApiAtParams {
                at,
                function: "AnchorApi_get_anchor_by_id",
                native_call,
                arguments: args,
                overlayed_changes: changes,
                storage_transaction_cache,
                context,
                recorder,
            };
            call_runtime_at.call_api_at(params)
        }
    }
    /// The API to query anchoring info.
    #[cfg(any(feature = "std", test))]
    pub trait AnchorApi<Block: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockT>:
        self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::Core<Block>
    {
        fn get_anchor_by_id(
            &self,
            __runtime_api_at_param__ : & self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: BlockId < Block >,
            id: Hash,
        ) -> std::result::Result<
            Option<AnchorData<Hash, BlockNumber>>,
            self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError,
        > {
            let runtime_api_impl_params_encoded =
                self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::Encode::encode(&(&id));
            self . AnchorApi_get_anchor_by_id_runtime_api_impl (__runtime_api_at_param__ , self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: ExecutionContext :: OffchainCall (None) , Some ((id)) , runtime_api_impl_params_encoded) . and_then (| r | match r { self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: NativeOrEncoded :: Native (n) => { Ok (n) } self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: NativeOrEncoded :: Encoded (r) => { < Option < AnchorData < Hash , BlockNumber > > as self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: Decode > :: decode (& mut & r [..]) . map_err (| err | self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: ApiError :: FailedToDecodeReturnValue { function : "get_anchor_by_id" , error : err , }) } })
        }
        fn get_anchor_by_id_with_context(
            &self,
            __runtime_api_at_param__ : & self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: BlockId < Block >,
            context: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ExecutionContext,
            id: Hash,
        ) -> std::result::Result<
            Option<AnchorData<Hash, BlockNumber>>,
            self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError,
        > {
            let runtime_api_impl_params_encoded =
                self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::Encode::encode(&(&id));
            self . AnchorApi_get_anchor_by_id_runtime_api_impl (__runtime_api_at_param__ , context , Some ((id)) , runtime_api_impl_params_encoded) . and_then (| r | match r { self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: NativeOrEncoded :: Native (n) => { Ok (n) } self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: NativeOrEncoded :: Encoded (r) => { < Option < AnchorData < Hash , BlockNumber > > as self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: Decode > :: decode (& mut & r [..]) . map_err (| err | self :: sp_api_hidden_includes_DECL_RUNTIME_APIS :: sp_api :: ApiError :: FailedToDecodeReturnValue { function : "get_anchor_by_id" , error : err , }) } })
        }
        #[doc(hidden)]
        fn AnchorApi_get_anchor_by_id_runtime_api_impl(
            &self,
            at: &self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockId<Block>,
            context: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ExecutionContext,
            params: Option<(Hash)>,
            params_encoded: Vec<u8>,
        ) -> std::result::Result<
            self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::NativeOrEncoded<
                Option<AnchorData<Hash, BlockNumber>>,
            >,
            self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::ApiError,
        >;
    }
    #[cfg(any(feature = "std", test))]
    impl<Block: self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::BlockT>
        self::sp_api_hidden_includes_DECL_RUNTIME_APIS::sp_api::RuntimeApiInfo
        for AnchorApi<Block>
    {
        const ID: [u8; 8] = [201u8, 55u8, 215u8, 156u8, 146u8, 196u8, 232u8, 114u8];
        const VERSION: u32 = 1u32;
    }
}
/// Common types for all runtimes
pub mod types {
    use scale_info::TypeInfo;
    #[cfg(feature = "std")]
    use serde::{Deserialize, Serialize};
    use sp_core::{H160, U256};
    use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, Verify};
    use sp_std::vec::Vec;
    /// An index to a block.
    pub type BlockNumber = u32;
    /// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
    pub type Signature = sp_runtime::MultiSignature;
    /// Some way of identifying an account on the chain. We intentionally make it equivalent
    /// to the public key of our transaction signing scheme.
    pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
    /// The type for looking up accounts. We don't expect more than 4 billion of them, but you
    /// never know...
    pub type AccountIndex = u32;
    /// The address format for describing accounts.
    pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
    /// Balance of an account.
    pub type Balance = u128;
    /// IBalance is the signed version of the Balance for orml tokens
    pub type IBalance = i128;
    /// Index of a transaction in the chain.
    pub type Index = u32;
    /// A hash of some data used by the chain.
    pub type Hash = sp_core::H256;
    /// Block header type as expected by this runtime.
    pub type Header = sp_runtime::generic::Header<BlockNumber, BlakeTwo256>;
    /// Digest item type.
    pub type DigestItem = sp_runtime::generic::DigestItem<Hash>;
    /// Aura consensus authority.
    pub type AuraId = sp_consensus_aura::sr25519::AuthorityId;
    /// Moment type
    pub type Moment = u64;
    pub type Bytes = Vec<u8>;
    pub type Bytes32 = FixedArray<u8, 32>;
    pub type FixedArray<T, const S: usize> = [T; S];
    pub type Salt = FixedArray<u8, 32>;
    /// A representation of registryID.
    pub struct RegistryId(pub H160);
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for RegistryId {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                RegistryId(ref __self_0_0) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "RegistryId");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
            }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for RegistryId {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                _serde::Serializer::serialize_newtype_struct(__serializer, "RegistryId", &self.0)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for RegistryId {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<RegistryId>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = RegistryId;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(
                            __formatter,
                            "tuple struct RegistryId",
                        )
                    }
                    #[inline]
                    fn visit_newtype_struct<__E>(
                        self,
                        __e: __E,
                    ) -> _serde::__private::Result<Self::Value, __E::Error>
                    where
                        __E: _serde::Deserializer<'de>,
                    {
                        let __field0: H160 = match <H160 as _serde::Deserialize>::deserialize(__e) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        _serde::__private::Ok(RegistryId(__field0))
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 =
                            match match _serde::de::SeqAccess::next_element::<H160>(&mut __seq) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"tuple struct RegistryId with 1 element",
                                        ),
                                    );
                                }
                            };
                        _serde::__private::Ok(RegistryId(__field0))
                    }
                }
                _serde::Deserializer::deserialize_newtype_struct(
                    __deserializer,
                    "RegistryId",
                    __Visitor {
                        marker: _serde::__private::PhantomData::<RegistryId>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    const _: () = {
        impl ::codec::Encode for RegistryId {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.0, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.0)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.0, f)
            }
        }
        impl ::codec::EncodeLike for RegistryId {}
    };
    const _: () = {
        impl ::codec::Decode for RegistryId {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(RegistryId({
                    let __codec_res_edqy = <H160 as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `RegistryId.0`"),
                            )
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                }))
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for RegistryId {
        #[inline]
        fn default() -> RegistryId {
            RegistryId(::core::default::Default::default())
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for RegistryId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for RegistryId {
        #[inline]
        fn clone(&self) -> RegistryId {
            {
                let _: ::core::clone::AssertParamIsClone<H160>;
                *self
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for RegistryId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for RegistryId {
        #[inline]
        fn eq(&self, other: &RegistryId) -> bool {
            match *other {
                RegistryId(ref __self_1_0) => match *self {
                    RegistryId(ref __self_0_0) => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &RegistryId) -> bool {
            match *other {
                RegistryId(ref __self_1_0) => match *self {
                    RegistryId(ref __self_0_0) => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for RegistryId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for RegistryId {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<H160>;
            }
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for RegistryId {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "RegistryId",
                        "runtime_common::types",
                    ))
                    .type_params(::alloc::vec::Vec::new())
                    .docs(&["A representation of registryID."])
                    .composite(
                        ::scale_info::build::Fields::unnamed()
                            .field(|f| f.ty::<H160>().type_name("H160").docs(&[])),
                    )
            }
        };
    };
    pub struct TokenId(pub U256);
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for TokenId {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                TokenId(ref __self_0_0) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "TokenId");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
            }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for TokenId {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                _serde::Serializer::serialize_newtype_struct(__serializer, "TokenId", &self.0)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for TokenId {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<TokenId>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = TokenId;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "tuple struct TokenId")
                    }
                    #[inline]
                    fn visit_newtype_struct<__E>(
                        self,
                        __e: __E,
                    ) -> _serde::__private::Result<Self::Value, __E::Error>
                    where
                        __E: _serde::Deserializer<'de>,
                    {
                        let __field0: U256 = match <U256 as _serde::Deserialize>::deserialize(__e) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        _serde::__private::Ok(TokenId(__field0))
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 =
                            match match _serde::de::SeqAccess::next_element::<U256>(&mut __seq) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"tuple struct TokenId with 1 element",
                                        ),
                                    );
                                }
                            };
                        _serde::__private::Ok(TokenId(__field0))
                    }
                }
                _serde::Deserializer::deserialize_newtype_struct(
                    __deserializer,
                    "TokenId",
                    __Visitor {
                        marker: _serde::__private::PhantomData::<TokenId>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    const _: () = {
        impl ::codec::Encode for TokenId {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.0, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.0)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.0, f)
            }
        }
        impl ::codec::EncodeLike for TokenId {}
    };
    const _: () = {
        impl ::codec::Decode for TokenId {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(TokenId({
                    let __codec_res_edqy = <U256 as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `TokenId.0`"),
                            )
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                }))
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for TokenId {
        #[inline]
        fn default() -> TokenId {
            TokenId(::core::default::Default::default())
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for TokenId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for TokenId {
        #[inline]
        fn clone(&self) -> TokenId {
            {
                let _: ::core::clone::AssertParamIsClone<U256>;
                *self
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for TokenId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for TokenId {
        #[inline]
        fn eq(&self, other: &TokenId) -> bool {
            match *other {
                TokenId(ref __self_1_0) => match *self {
                    TokenId(ref __self_0_0) => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &TokenId) -> bool {
            match *other {
                TokenId(ref __self_1_0) => match *self {
                    TokenId(ref __self_0_0) => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for TokenId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for TokenId {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<U256>;
            }
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for TokenId {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new("TokenId", "runtime_common::types"))
                    .type_params(::alloc::vec::Vec::new())
                    .docs(&[])
                    .composite(
                        ::scale_info::build::Fields::unnamed()
                            .field(|f| f.ty::<U256>().type_name("U256").docs(&[])),
                    )
            }
        };
    };
    /// A generic representation of a local address. A resource id points to this. It may be a
    /// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
    /// as an upper bound to store efficiently.
    pub struct EthAddress(pub Bytes32);
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for EthAddress {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                EthAddress(ref __self_0_0) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "EthAddress");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
            }
        }
    }
    const _: () = {
        impl ::codec::Encode for EthAddress {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.0, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.0)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.0, f)
            }
        }
        impl ::codec::EncodeLike for EthAddress {}
    };
    const _: () = {
        impl ::codec::Decode for EthAddress {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(EthAddress({
                    let __codec_res_edqy = <Bytes32 as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `EthAddress.0`"),
                            )
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                }))
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for EthAddress {
        #[inline]
        fn default() -> EthAddress {
            EthAddress(::core::default::Default::default())
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for EthAddress {
        #[inline]
        fn clone(&self) -> EthAddress {
            match *self {
                EthAddress(ref __self_0_0) => {
                    EthAddress(::core::clone::Clone::clone(&(*__self_0_0)))
                }
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for EthAddress {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for EthAddress {
        #[inline]
        fn eq(&self, other: &EthAddress) -> bool {
            match *other {
                EthAddress(ref __self_1_0) => match *self {
                    EthAddress(ref __self_0_0) => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &EthAddress) -> bool {
            match *other {
                EthAddress(ref __self_1_0) => match *self {
                    EthAddress(ref __self_0_0) => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for EthAddress {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for EthAddress {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<Bytes32>;
            }
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for EthAddress {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                :: scale_info :: Type :: builder () . path (:: scale_info :: Path :: new ("EthAddress" , "runtime_common::types")) . type_params (:: alloc :: vec :: Vec :: new ()) . docs (& ["A generic representation of a local address. A resource id points to this. It may be a" , "registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just" , "as an upper bound to store efficiently."]) . composite (:: scale_info :: build :: Fields :: unnamed () . field (| f | f . ty :: < Bytes32 > () . type_name ("Bytes32") . docs (& [])))
            }
        };
    };
    /// Rate with 27 precision fixed point decimal
    pub type Rate = crate::fixed_point::Rate;
    /// Amount with 18 precision fixed point decimal
    pub type Amount = crate::fixed_point::Amount;
    /// PoolId type we use.
    pub type PoolId = u64;
    /// A representation of ClassId for Uniques
    pub type ClassId = u64;
    /// A representation of InstanceId for Uniques.
    pub struct InstanceId(pub u128);
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for InstanceId {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                _serde::Serializer::serialize_newtype_struct(__serializer, "InstanceId", &self.0)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for InstanceId {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<InstanceId>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = InstanceId;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(
                            __formatter,
                            "tuple struct InstanceId",
                        )
                    }
                    #[inline]
                    fn visit_newtype_struct<__E>(
                        self,
                        __e: __E,
                    ) -> _serde::__private::Result<Self::Value, __E::Error>
                    where
                        __E: _serde::Deserializer<'de>,
                    {
                        let __field0: u128 = match <u128 as _serde::Deserialize>::deserialize(__e) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        };
                        _serde::__private::Ok(InstanceId(__field0))
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 =
                            match match _serde::de::SeqAccess::next_element::<u128>(&mut __seq) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"tuple struct InstanceId with 1 element",
                                        ),
                                    );
                                }
                            };
                        _serde::__private::Ok(InstanceId(__field0))
                    }
                }
                _serde::Deserializer::deserialize_newtype_struct(
                    __deserializer,
                    "InstanceId",
                    __Visitor {
                        marker: _serde::__private::PhantomData::<InstanceId>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    const _: () = {
        impl ::codec::Encode for InstanceId {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.0, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.0)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.0, f)
            }
        }
        impl ::codec::EncodeLike for InstanceId {}
    };
    const _: () = {
        impl ::codec::Decode for InstanceId {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(InstanceId({
                    let __codec_res_edqy = <u128 as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `InstanceId.0`"),
                            )
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                }))
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for InstanceId {
        #[inline]
        fn default() -> InstanceId {
            InstanceId(::core::default::Default::default())
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for InstanceId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for InstanceId {
        #[inline]
        fn clone(&self) -> InstanceId {
            {
                let _: ::core::clone::AssertParamIsClone<u128>;
                *self
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for InstanceId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for InstanceId {
        #[inline]
        fn eq(&self, other: &InstanceId) -> bool {
            match *other {
                InstanceId(ref __self_1_0) => match *self {
                    InstanceId(ref __self_0_0) => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &InstanceId) -> bool {
            match *other {
                InstanceId(ref __self_1_0) => match *self {
                    InstanceId(ref __self_0_0) => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for InstanceId {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for InstanceId {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<u128>;
            }
        }
    }
    const _: () = {
        impl ::codec::CompactAs for InstanceId {
            type As = u128;
            fn encode_as(&self) -> &u128 {
                &self.0
            }
            fn decode_from(x: u128) -> ::core::result::Result<InstanceId, ::codec::Error> {
                ::core::result::Result::Ok(InstanceId(x))
            }
        }
        impl From<::codec::Compact<InstanceId>> for InstanceId {
            fn from(x: ::codec::Compact<InstanceId>) -> InstanceId {
                x.0
            }
        }
    };
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for InstanceId {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                InstanceId(ref __self_0_0) => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_tuple(f, "InstanceId");
                    let _ = ::core::fmt::DebugTuple::field(debug_trait_builder, &&(*__self_0_0));
                    ::core::fmt::DebugTuple::finish(debug_trait_builder)
                }
            }
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for InstanceId {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "InstanceId",
                        "runtime_common::types",
                    ))
                    .type_params(::alloc::vec::Vec::new())
                    .docs(&["A representation of InstanceId for Uniques."])
                    .composite(
                        ::scale_info::build::Fields::unnamed()
                            .field(|f| f.ty::<u128>().type_name("u128").docs(&[])),
                    )
            }
        };
    };
    pub struct PoolRoles {
        bits: u32,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for PoolRoles {}
    impl ::core::marker::StructuralPartialEq for PoolRoles {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for PoolRoles {
        #[inline]
        fn eq(&self, other: &PoolRoles) -> bool {
            match *other {
                PoolRoles {
                    bits: ref __self_1_0,
                } => match *self {
                    PoolRoles {
                        bits: ref __self_0_0,
                    } => (*__self_0_0) == (*__self_1_0),
                },
            }
        }
        #[inline]
        fn ne(&self, other: &PoolRoles) -> bool {
            match *other {
                PoolRoles {
                    bits: ref __self_1_0,
                } => match *self {
                    PoolRoles {
                        bits: ref __self_0_0,
                    } => (*__self_0_0) != (*__self_1_0),
                },
            }
        }
    }
    impl ::core::marker::StructuralEq for PoolRoles {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Eq for PoolRoles {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            {
                let _: ::core::cmp::AssertParamIsEq<u32>;
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for PoolRoles {
        #[inline]
        fn clone(&self) -> PoolRoles {
            {
                let _: ::core::clone::AssertParamIsClone<u32>;
                *self
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialOrd for PoolRoles {
        #[inline]
        fn partial_cmp(&self, other: &PoolRoles) -> ::core::option::Option<::core::cmp::Ordering> {
            match *other {
                PoolRoles {
                    bits: ref __self_1_0,
                } => match *self {
                    PoolRoles {
                        bits: ref __self_0_0,
                    } => match ::core::cmp::PartialOrd::partial_cmp(&(*__self_0_0), &(*__self_1_0))
                    {
                        ::core::option::Option::Some(::core::cmp::Ordering::Equal) => {
                            ::core::option::Option::Some(::core::cmp::Ordering::Equal)
                        }
                        cmp => cmp,
                    },
                },
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::Ord for PoolRoles {
        #[inline]
        fn cmp(&self, other: &PoolRoles) -> ::core::cmp::Ordering {
            match *other {
                PoolRoles {
                    bits: ref __self_1_0,
                } => match *self {
                    PoolRoles {
                        bits: ref __self_0_0,
                    } => match ::core::cmp::Ord::cmp(&(*__self_0_0), &(*__self_1_0)) {
                        ::core::cmp::Ordering::Equal => ::core::cmp::Ordering::Equal,
                        cmp => cmp,
                    },
                },
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::hash::Hash for PoolRoles {
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            match *self {
                PoolRoles {
                    bits: ref __self_0_0,
                } => ::core::hash::Hash::hash(&(*__self_0_0), state),
            }
        }
    }
    const _: () = {
        impl ::codec::Encode for PoolRoles {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&&self.bits, __codec_dest_edqy)
            }
            fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
                ::codec::Encode::encode(&&self.bits)
            }
            fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
                &self,
                f: F,
            ) -> R {
                ::codec::Encode::using_encoded(&&self.bits, f)
            }
        }
        impl ::codec::EncodeLike for PoolRoles {}
    };
    const _: () = {
        impl ::codec::Decode for PoolRoles {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(PoolRoles {
                    bits: {
                        let __codec_res_edqy = <u32 as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `PoolRoles::bits`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                })
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for PoolRoles {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "PoolRoles",
                        "runtime_common::types",
                    ))
                    .type_params(::alloc::vec::Vec::new())
                    .docs(&[])
                    .composite(
                        ::scale_info::build::Fields::named()
                            .field(|f| f.ty::<u32>().name("bits").type_name("u32").docs(&[])),
                    )
            }
        };
    };
    impl ::bitflags::_core::fmt::Debug for PoolRoles {
        fn fmt(&self, f: &mut ::bitflags::_core::fmt::Formatter) -> ::bitflags::_core::fmt::Result {
            #[allow(non_snake_case)]
            trait __BitFlags {
                #[inline]
                fn POOL_ADMIN(&self) -> bool {
                    false
                }
                #[inline]
                fn BORROWER(&self) -> bool {
                    false
                }
                #[inline]
                fn PRICING_ADMIN(&self) -> bool {
                    false
                }
                #[inline]
                fn LIQUIDITY_ADMIN(&self) -> bool {
                    false
                }
                #[inline]
                fn MEMBER_LIST_ADMIN(&self) -> bool {
                    false
                }
                #[inline]
                fn RISK_ADMIN(&self) -> bool {
                    false
                }
            }
            #[allow(non_snake_case)]
            impl __BitFlags for PoolRoles {
                #[allow(deprecated)]
                #[inline]
                fn POOL_ADMIN(&self) -> bool {
                    if Self::POOL_ADMIN.bits == 0 && self.bits != 0 {
                        false
                    } else {
                        self.bits & Self::POOL_ADMIN.bits == Self::POOL_ADMIN.bits
                    }
                }
                #[allow(deprecated)]
                #[inline]
                fn BORROWER(&self) -> bool {
                    if Self::BORROWER.bits == 0 && self.bits != 0 {
                        false
                    } else {
                        self.bits & Self::BORROWER.bits == Self::BORROWER.bits
                    }
                }
                #[allow(deprecated)]
                #[inline]
                fn PRICING_ADMIN(&self) -> bool {
                    if Self::PRICING_ADMIN.bits == 0 && self.bits != 0 {
                        false
                    } else {
                        self.bits & Self::PRICING_ADMIN.bits == Self::PRICING_ADMIN.bits
                    }
                }
                #[allow(deprecated)]
                #[inline]
                fn LIQUIDITY_ADMIN(&self) -> bool {
                    if Self::LIQUIDITY_ADMIN.bits == 0 && self.bits != 0 {
                        false
                    } else {
                        self.bits & Self::LIQUIDITY_ADMIN.bits == Self::LIQUIDITY_ADMIN.bits
                    }
                }
                #[allow(deprecated)]
                #[inline]
                fn MEMBER_LIST_ADMIN(&self) -> bool {
                    if Self::MEMBER_LIST_ADMIN.bits == 0 && self.bits != 0 {
                        false
                    } else {
                        self.bits & Self::MEMBER_LIST_ADMIN.bits == Self::MEMBER_LIST_ADMIN.bits
                    }
                }
                #[allow(deprecated)]
                #[inline]
                fn RISK_ADMIN(&self) -> bool {
                    if Self::RISK_ADMIN.bits == 0 && self.bits != 0 {
                        false
                    } else {
                        self.bits & Self::RISK_ADMIN.bits == Self::RISK_ADMIN.bits
                    }
                }
            }
            let mut first = true;
            if <Self as __BitFlags>::POOL_ADMIN(self) {
                if !first {
                    f.write_str(" | ")?;
                }
                first = false;
                f.write_str("POOL_ADMIN")?;
            }
            if <Self as __BitFlags>::BORROWER(self) {
                if !first {
                    f.write_str(" | ")?;
                }
                first = false;
                f.write_str("BORROWER")?;
            }
            if <Self as __BitFlags>::PRICING_ADMIN(self) {
                if !first {
                    f.write_str(" | ")?;
                }
                first = false;
                f.write_str("PRICING_ADMIN")?;
            }
            if <Self as __BitFlags>::LIQUIDITY_ADMIN(self) {
                if !first {
                    f.write_str(" | ")?;
                }
                first = false;
                f.write_str("LIQUIDITY_ADMIN")?;
            }
            if <Self as __BitFlags>::MEMBER_LIST_ADMIN(self) {
                if !first {
                    f.write_str(" | ")?;
                }
                first = false;
                f.write_str("MEMBER_LIST_ADMIN")?;
            }
            if <Self as __BitFlags>::RISK_ADMIN(self) {
                if !first {
                    f.write_str(" | ")?;
                }
                first = false;
                f.write_str("RISK_ADMIN")?;
            }
            let extra_bits = self.bits & !Self::all().bits();
            if extra_bits != 0 {
                if !first {
                    f.write_str(" | ")?;
                }
                first = false;
                f.write_str("0x")?;
                ::bitflags::_core::fmt::LowerHex::fmt(&extra_bits, f)?;
            }
            if first {
                f.write_str("(empty)")?;
            }
            Ok(())
        }
    }
    impl ::bitflags::_core::fmt::Binary for PoolRoles {
        fn fmt(&self, f: &mut ::bitflags::_core::fmt::Formatter) -> ::bitflags::_core::fmt::Result {
            ::bitflags::_core::fmt::Binary::fmt(&self.bits, f)
        }
    }
    impl ::bitflags::_core::fmt::Octal for PoolRoles {
        fn fmt(&self, f: &mut ::bitflags::_core::fmt::Formatter) -> ::bitflags::_core::fmt::Result {
            ::bitflags::_core::fmt::Octal::fmt(&self.bits, f)
        }
    }
    impl ::bitflags::_core::fmt::LowerHex for PoolRoles {
        fn fmt(&self, f: &mut ::bitflags::_core::fmt::Formatter) -> ::bitflags::_core::fmt::Result {
            ::bitflags::_core::fmt::LowerHex::fmt(&self.bits, f)
        }
    }
    impl ::bitflags::_core::fmt::UpperHex for PoolRoles {
        fn fmt(&self, f: &mut ::bitflags::_core::fmt::Formatter) -> ::bitflags::_core::fmt::Result {
            ::bitflags::_core::fmt::UpperHex::fmt(&self.bits, f)
        }
    }
    #[allow(dead_code)]
    impl PoolRoles {
        pub const POOL_ADMIN: Self = Self { bits: 0b00000001 };
        pub const BORROWER: Self = Self { bits: 0b00000010 };
        pub const PRICING_ADMIN: Self = Self { bits: 0b00000100 };
        pub const LIQUIDITY_ADMIN: Self = Self { bits: 0b00001000 };
        pub const MEMBER_LIST_ADMIN: Self = Self { bits: 0b00010000 };
        pub const RISK_ADMIN: Self = Self { bits: 0b00100000 };
        /// Returns an empty set of flags.
        #[inline]
        pub const fn empty() -> Self {
            Self { bits: 0 }
        }
        /// Returns the set containing all flags.
        #[inline]
        pub const fn all() -> Self {
            #[allow(non_snake_case)]
            trait __BitFlags {
                const POOL_ADMIN: u32 = 0;
                const BORROWER: u32 = 0;
                const PRICING_ADMIN: u32 = 0;
                const LIQUIDITY_ADMIN: u32 = 0;
                const MEMBER_LIST_ADMIN: u32 = 0;
                const RISK_ADMIN: u32 = 0;
            }
            #[allow(non_snake_case)]
            impl __BitFlags for PoolRoles {
                #[allow(deprecated)]
                const POOL_ADMIN: u32 = Self::POOL_ADMIN.bits;
                #[allow(deprecated)]
                const BORROWER: u32 = Self::BORROWER.bits;
                #[allow(deprecated)]
                const PRICING_ADMIN: u32 = Self::PRICING_ADMIN.bits;
                #[allow(deprecated)]
                const LIQUIDITY_ADMIN: u32 = Self::LIQUIDITY_ADMIN.bits;
                #[allow(deprecated)]
                const MEMBER_LIST_ADMIN: u32 = Self::MEMBER_LIST_ADMIN.bits;
                #[allow(deprecated)]
                const RISK_ADMIN: u32 = Self::RISK_ADMIN.bits;
            }
            Self {
                bits: <Self as __BitFlags>::POOL_ADMIN
                    | <Self as __BitFlags>::BORROWER
                    | <Self as __BitFlags>::PRICING_ADMIN
                    | <Self as __BitFlags>::LIQUIDITY_ADMIN
                    | <Self as __BitFlags>::MEMBER_LIST_ADMIN
                    | <Self as __BitFlags>::RISK_ADMIN,
            }
        }
        /// Returns the raw value of the flags currently stored.
        #[inline]
        pub const fn bits(&self) -> u32 {
            self.bits
        }
        /// Convert from underlying bit representation, unless that
        /// representation contains bits that do not correspond to a flag.
        #[inline]
        pub const fn from_bits(bits: u32) -> ::bitflags::_core::option::Option<Self> {
            if (bits & !Self::all().bits()) == 0 {
                ::bitflags::_core::option::Option::Some(Self { bits })
            } else {
                ::bitflags::_core::option::Option::None
            }
        }
        /// Convert from underlying bit representation, dropping any bits
        /// that do not correspond to flags.
        #[inline]
        pub const fn from_bits_truncate(bits: u32) -> Self {
            Self {
                bits: bits & Self::all().bits,
            }
        }
        /// Convert from underlying bit representation, preserving all
        /// bits (even those not corresponding to a defined flag).
        ///
        /// # Safety
        ///
        /// The caller of the `bitflags!` macro can chose to allow or
        /// disallow extra bits for their bitflags type.
        ///
        /// The caller of `from_bits_unchecked()` has to ensure that
        /// all bits correspond to a defined flag or that extra bits
        /// are valid for this bitflags type.
        #[inline]
        pub const unsafe fn from_bits_unchecked(bits: u32) -> Self {
            Self { bits }
        }
        /// Returns `true` if no flags are currently stored.
        #[inline]
        pub const fn is_empty(&self) -> bool {
            self.bits() == Self::empty().bits()
        }
        /// Returns `true` if all flags are currently set.
        #[inline]
        pub const fn is_all(&self) -> bool {
            Self::all().bits | self.bits == self.bits
        }
        /// Returns `true` if there are flags common to both `self` and `other`.
        #[inline]
        pub const fn intersects(&self, other: Self) -> bool {
            !(Self {
                bits: self.bits & other.bits,
            })
            .is_empty()
        }
        /// Returns `true` if all of the flags in `other` are contained within `self`.
        #[inline]
        pub const fn contains(&self, other: Self) -> bool {
            (self.bits & other.bits) == other.bits
        }
        /// Inserts the specified flags in-place.
        #[inline]
        pub fn insert(&mut self, other: Self) {
            self.bits |= other.bits;
        }
        /// Removes the specified flags in-place.
        #[inline]
        pub fn remove(&mut self, other: Self) {
            self.bits &= !other.bits;
        }
        /// Toggles the specified flags in-place.
        #[inline]
        pub fn toggle(&mut self, other: Self) {
            self.bits ^= other.bits;
        }
        /// Inserts or removes the specified flags depending on the passed value.
        #[inline]
        pub fn set(&mut self, other: Self, value: bool) {
            if value {
                self.insert(other);
            } else {
                self.remove(other);
            }
        }
        /// Returns the intersection between the flags in `self` and
        /// `other`.
        ///
        /// Specifically, the returned set contains only the flags which are
        /// present in *both* `self` *and* `other`.
        ///
        /// This is equivalent to using the `&` operator (e.g.
        /// [`ops::BitAnd`]), as in `flags & other`.
        ///
        /// [`ops::BitAnd`]: https://doc.rust-lang.org/std/ops/trait.BitAnd.html
        #[inline]
        #[must_use]
        pub const fn intersection(self, other: Self) -> Self {
            Self {
                bits: self.bits & other.bits,
            }
        }
        /// Returns the union of between the flags in `self` and `other`.
        ///
        /// Specifically, the returned set contains all flags which are
        /// present in *either* `self` *or* `other`, including any which are
        /// present in both (see [`Self::symmetric_difference`] if that
        /// is undesirable).
        ///
        /// This is equivalent to using the `|` operator (e.g.
        /// [`ops::BitOr`]), as in `flags | other`.
        ///
        /// [`ops::BitOr`]: https://doc.rust-lang.org/std/ops/trait.BitOr.html
        #[inline]
        #[must_use]
        pub const fn union(self, other: Self) -> Self {
            Self {
                bits: self.bits | other.bits,
            }
        }
        /// Returns the difference between the flags in `self` and `other`.
        ///
        /// Specifically, the returned set contains all flags present in
        /// `self`, except for the ones present in `other`.
        ///
        /// It is also conceptually equivalent to the "bit-clear" operation:
        /// `flags & !other` (and this syntax is also supported).
        ///
        /// This is equivalent to using the `-` operator (e.g.
        /// [`ops::Sub`]), as in `flags - other`.
        ///
        /// [`ops::Sub`]: https://doc.rust-lang.org/std/ops/trait.Sub.html
        #[inline]
        #[must_use]
        pub const fn difference(self, other: Self) -> Self {
            Self {
                bits: self.bits & !other.bits,
            }
        }
        /// Returns the [symmetric difference][sym-diff] between the flags
        /// in `self` and `other`.
        ///
        /// Specifically, the returned set contains the flags present which
        /// are present in `self` or `other`, but that are not present in
        /// both. Equivalently, it contains the flags present in *exactly
        /// one* of the sets `self` and `other`.
        ///
        /// This is equivalent to using the `^` operator (e.g.
        /// [`ops::BitXor`]), as in `flags ^ other`.
        ///
        /// [sym-diff]: https://en.wikipedia.org/wiki/Symmetric_difference
        /// [`ops::BitXor`]: https://doc.rust-lang.org/std/ops/trait.BitXor.html
        #[inline]
        #[must_use]
        pub const fn symmetric_difference(self, other: Self) -> Self {
            Self {
                bits: self.bits ^ other.bits,
            }
        }
        /// Returns the complement of this set of flags.
        ///
        /// Specifically, the returned set contains all the flags which are
        /// not set in `self`, but which are allowed for this type.
        ///
        /// Alternatively, it can be thought of as the set difference
        /// between [`Self::all()`] and `self` (e.g. `Self::all() - self`)
        ///
        /// This is equivalent to using the `!` operator (e.g.
        /// [`ops::Not`]), as in `!flags`.
        ///
        /// [`Self::all()`]: Self::all
        /// [`ops::Not`]: https://doc.rust-lang.org/std/ops/trait.Not.html
        #[inline]
        #[must_use]
        pub const fn complement(self) -> Self {
            Self::from_bits_truncate(!self.bits)
        }
    }
    impl ::bitflags::_core::ops::BitOr for PoolRoles {
        type Output = Self;
        /// Returns the union of the two sets of flags.
        #[inline]
        fn bitor(self, other: PoolRoles) -> Self {
            Self {
                bits: self.bits | other.bits,
            }
        }
    }
    impl ::bitflags::_core::ops::BitOrAssign for PoolRoles {
        /// Adds the set of flags.
        #[inline]
        fn bitor_assign(&mut self, other: Self) {
            self.bits |= other.bits;
        }
    }
    impl ::bitflags::_core::ops::BitXor for PoolRoles {
        type Output = Self;
        /// Returns the left flags, but with all the right flags toggled.
        #[inline]
        fn bitxor(self, other: Self) -> Self {
            Self {
                bits: self.bits ^ other.bits,
            }
        }
    }
    impl ::bitflags::_core::ops::BitXorAssign for PoolRoles {
        /// Toggles the set of flags.
        #[inline]
        fn bitxor_assign(&mut self, other: Self) {
            self.bits ^= other.bits;
        }
    }
    impl ::bitflags::_core::ops::BitAnd for PoolRoles {
        type Output = Self;
        /// Returns the intersection between the two sets of flags.
        #[inline]
        fn bitand(self, other: Self) -> Self {
            Self {
                bits: self.bits & other.bits,
            }
        }
    }
    impl ::bitflags::_core::ops::BitAndAssign for PoolRoles {
        /// Disables all flags disabled in the set.
        #[inline]
        fn bitand_assign(&mut self, other: Self) {
            self.bits &= other.bits;
        }
    }
    impl ::bitflags::_core::ops::Sub for PoolRoles {
        type Output = Self;
        /// Returns the set difference of the two sets of flags.
        #[inline]
        fn sub(self, other: Self) -> Self {
            Self {
                bits: self.bits & !other.bits,
            }
        }
    }
    impl ::bitflags::_core::ops::SubAssign for PoolRoles {
        /// Disables all flags enabled in the set.
        #[inline]
        fn sub_assign(&mut self, other: Self) {
            self.bits &= !other.bits;
        }
    }
    impl ::bitflags::_core::ops::Not for PoolRoles {
        type Output = Self;
        /// Returns the complement of this set of flags.
        #[inline]
        fn not(self) -> Self {
            Self { bits: !self.bits } & Self::all()
        }
    }
    impl ::bitflags::_core::iter::Extend<PoolRoles> for PoolRoles {
        fn extend<T: ::bitflags::_core::iter::IntoIterator<Item = Self>>(&mut self, iterator: T) {
            for item in iterator {
                self.insert(item)
            }
        }
    }
    impl ::bitflags::_core::iter::FromIterator<PoolRoles> for PoolRoles {
        fn from_iter<T: ::bitflags::_core::iter::IntoIterator<Item = Self>>(iterator: T) -> Self {
            let mut result = Self::empty();
            result.extend(iterator);
            result
        }
    }
}
/// Common constants for all runtimes
pub mod constants {
    use super::types::BlockNumber;
    use frame_support::weights::{constants::WEIGHT_PER_SECOND, Weight};
    use node_primitives::Balance;
    use sp_runtime::Perbill;
    /// This determines the average expected block time that we are targeting. Blocks will be
    /// produced at a minimum duration defined by `SLOT_DURATION`. `SLOT_DURATION` is picked up by
    /// `pallet_timestamp` which is in turn picked up by `pallet_aura` to implement `fn
    /// slot_duration()`.
    ///
    /// Change this to adjust the block time.
    pub const MILLISECS_PER_BLOCK: u64 = 12000;
    pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;
    pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
    pub const HOURS: BlockNumber = MINUTES * 60;
    pub const DAYS: BlockNumber = HOURS * 24;
    /// Milliseconds per day
    pub const MILLISECS_PER_DAY: u64 = 86400000;
    /// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
    /// used to limit the maximal weight of a single extrinsic.
    pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);
    /// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
    /// Operational  extrinsics.
    pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
    /// We allow for 0.5 seconds of compute with a 6 second average block time.
    pub const MAXIMUM_BLOCK_WEIGHT: Weight = WEIGHT_PER_SECOND / 2;
    pub const MICRO_CFG: Balance = 1_000_000_000_000;
    pub const MILLI_CFG: Balance = 1_000 * MICRO_CFG;
    pub const CENTI_CFG: Balance = 10 * MILLI_CFG;
    pub const CFG: Balance = 100 * CENTI_CFG;
    /// Minimum vesting amount, in CFG/AIR
    pub const MIN_VESTING: Balance = 10;
    /// Additional fee charged when moving native tokens to target chains (in CFGs).
    pub const NATIVE_TOKEN_TRANSFER_FEE: Balance = 2000 * CFG;
    /// Additional fee charged when moving NFTs to target chains (in CFGs).
    pub const NFT_TOKEN_TRANSFER_FEE: Balance = 20 * CFG;
    /// Additional fee charged when validating NFT proofs
    pub const NFT_PROOF_VALIDATION_FEE: Balance = 10 * CFG;
    /// These are pre/appended to the registry id before being set as a [RegistryInfo] field in [create_registry].
    pub const NFTS_PREFIX: &'static [u8] = &[1, 0, 0, 0, 0, 0, 0, 20];
    pub const fn deposit(items: u32, bytes: u32) -> Balance {
        items as Balance * 15 * CENTI_CFG + (bytes as Balance) * 6 * CENTI_CFG
    }
}
