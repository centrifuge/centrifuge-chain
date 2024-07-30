use crate::num_wrapper::NumWrapper;

/// Trait to convert into seconds
pub trait IntoSeconds {
	type Seconds;
	fn into_seconds(self) -> Self::Seconds;
}

/// Trait to convert into millis
pub trait IntoMillis {
	type Millis;
	fn into_millis(self) -> Self::Millis;
}

/// Type to distinguish NumWrapper as millis
pub struct MillisId;

/// Type to represent milliseconds
pub type Millis<T> = NumWrapper<T, MillisId>;

macro_rules! into_seconds {
	($type_name:ident < $t:ty >) => {
		impl $type_name<$t> {
			pub const fn into_seconds(self) -> Seconds<$t> {
				Seconds::from(self.get() / 1000)
			}
		}

		impl IntoSeconds for $type_name<$t> {
			type Seconds = Seconds<$t>;

			fn into_seconds(self) -> Seconds<$t> {
				self.into_seconds()
			}
		}
	};
}

into_seconds!(Millis<u32>);
into_seconds!(Millis<u64>);
into_seconds!(Millis<u128>);

/// Type to distinguish NumWrapper as seconds
pub struct SecondsId;

/// Type to represent seconds
pub type Seconds<T> = NumWrapper<T, SecondsId>;

macro_rules! into_millis {
	($type_name:ident < $t:ty >) => {
		impl $type_name<$t> {
			pub const fn into_millis(self) -> Millis<$t> {
				Millis::from(self.get().saturating_mul(1000))
			}
		}

		impl IntoMillis for $type_name<$t> {
			type Millis = Millis<$t>;

			fn into_millis(self) -> Millis<$t> {
				self.into_millis()
			}
		}
	};
}

into_millis!(Seconds<u32>);
into_millis!(Seconds<u64>);
into_millis!(Seconds<u128>);
