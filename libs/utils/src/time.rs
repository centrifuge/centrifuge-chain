use sp_arithmetic::traits::{Bounded, Saturating};
use sp_std::ops::Div;

use crate::num_wrapper::NumWrapper;

/// Type to distinguish NumWrapper as millis
pub struct MillisId;

/// Type to represent milliseconds
pub type Millis<T> = NumWrapper<T, MillisId>;

impl<M: Div<Output = M> + From<u32> + Copy> Millis<M> {
	pub fn into_seconds<S: TryFrom<M> + Bounded>(self) -> Seconds<S> {
		let inner = self.inner / M::from(1000);
		Seconds::new(S::try_from(inner).unwrap_or(Bounded::max_value()))
	}

	pub fn into_days<D: TryFrom<M> + Bounded>(self) -> Days<D> {
		let inner = self.inner / M::from(1000 * 24 * 3600);
		Days::new(D::try_from(inner).unwrap_or(Bounded::max_value()))
	}
}

/// Type to distinguish NumWrapper as seconds
pub struct SecondsId;

/// Type to represent seconds
pub type Seconds<T> = NumWrapper<T, SecondsId>;

impl<S: Copy> Seconds<S> {
	pub fn into_millis<M: TryFrom<S> + From<u32> + Saturating + Bounded>(self) -> Millis<M> {
		let inner = M::try_from(self.inner)
			.unwrap_or(Bounded::max_value())
			.saturating_mul(M::from(1000));
		Millis::new(inner)
	}
}

impl<S: Div<Output = S> + From<u32> + Copy> Seconds<S> {
	pub fn into_days<D: TryFrom<S> + Bounded>(self) -> Days<D> {
		let inner = self.inner / S::from(24 * 3600);
		Days::new(D::try_from(inner).unwrap_or(Bounded::max_value()))
	}
}

/// Type to distinguish NumWrapper as days
pub struct DaysId;

/// Type to represent days
pub type Days<T> = NumWrapper<T, DaysId>;

impl<D: Copy> Days<D> {
	pub fn into_millis<M: TryFrom<D> + From<u32> + Saturating + Bounded>(self) -> Millis<M> {
		let inner = M::try_from(self.inner)
			.unwrap_or(Bounded::max_value())
			.saturating_mul(M::from(1000 * 24 * 3600));
		Millis::new(inner)
	}

	pub fn into_seconds<S: TryFrom<D> + From<u32> + Saturating + Bounded>(self) -> Seconds<S> {
		let inner = S::try_from(self.inner)
			.unwrap_or(Bounded::max_value())
			.saturating_mul(S::from(24 * 3600));
		Seconds::new(inner)
	}
}

// TODO: evaluate if we need a macro here. Some thoughts below.
// Could we get the above method as const with the macro?

/*
time_coversion_down!(into_millis, Seconds, Millis, factor)
time_coversion_down!(into_millis, Days, Millis, factor)

time_coversion_up!(into_seconds, Millis, Seconds, factor)
time_coversion_down!(into_seconds, Days, Seconds, factor)

time_coversion_up!(into_days, Millis, Days, factor)
time_coversion_up!(into_days, Seconds, Days, factor)
*/

/*
macro_rules! into_unit {
	($from_name:ident < $from:ty >, $to_name:ident < $to:ty >, $method_name:ident, $n:expr, $d:expr) => {
		impl $from_name<$from> {
			pub const fn $method_name(self) -> $to_name<$to> {
				let n: $to = $n as $to;
				$to_name::from((self.get() as $to).saturating_mul(n) / $d)
			}
		}
	};

	($from_name:ident < $from:ty >, $to_name:ident < $to:ty >, $method_name:ident, $n:expr, $d:expr, try) => {
		impl $from_name<$from> {
			pub fn $method_name(self) -> Result<$to_name<$to>, ArithmeticError> {
				$to_name::ensure_from(self.get().saturating_mul($n) / $d)
			}
		}
	};
}
*/
