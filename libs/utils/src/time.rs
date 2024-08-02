use sp_arithmetic::{traits::EnsureFrom, ArithmeticError};

use crate::num_wrapper::NumWrapper;

/// Type to distinguish NumWrapper as millis
pub struct MillisId;

/// Type to represent milliseconds
pub type Millis<T> = NumWrapper<T, MillisId>;

/// Type to distinguish NumWrapper as seconds
pub struct SecondsId;

/// Type to represent seconds
pub type Seconds<T> = NumWrapper<T, SecondsId>;

/// Type to distinguish NumWrapper as days
pub struct DaysId;

/// Type to represent days
pub type Days<T> = NumWrapper<T, DaysId>;

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

into_unit!(Millis<u64>, Seconds<u64>, into_seconds, 1, 1000);
into_unit!(
	Millis<u64>,
	Days<u32>,
	try_into_days,
	1,
	1000 * 24 * 3600,
	try
);

into_unit!(Seconds<u64>, Millis<u64>, into_millis, 1000, 1);
into_unit!(Seconds<u64>, Days<u32>, try_into_days, 1, 24 * 3600, try);

into_unit!(Days<u32>, Millis<u64>, into_millis, 1000 * 24 * 3600, 1);
into_unit!(Days<u64>, Seconds<u32>, into_seconds, 1, 1000 * 24 * 3600);
