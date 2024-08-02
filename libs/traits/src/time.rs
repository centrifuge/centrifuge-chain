use cfg_primitives::{Millis, Seconds};

/// Trait to obtain the unix time as seconds
pub trait UnixTimeSecs: UnixTime {
	fn now() -> Seconds {
		<Self as UnixTime>::now().as_secs().into()
	}

	/// Same as now(), shortcut for cases where `now()` conflicts with
	/// `UnixTime::now()`
	fn now_secs() -> Seconds {
		<Self as UnixTimeSecs>::now()
	}
}

impl<T: UnixTime> UnixTimeSecs for T {}

/// Trait to handle a time unit transparetly
pub trait TimeUnit {
	type Millis;
	type Seconds;

	fn into_millis(self) -> Self::Millis;
	fn into_seconds(self) -> Self::Seconds;
}

impl TimeUnit for Millis {
	type Millis = Millis;
	type Seconds = Seconds;

	fn into_millis(self) -> Self::Millis {
		self
	}

	fn into_seconds(self) -> Self::Seconds {
		self.into_seconds()
	}
}

impl TimeUnit for Seconds {
	type Millis = Millis;
	type Seconds = Seconds;

	fn into_millis(self) -> Self::Millis {
		self.into_millis()
	}

	fn into_seconds(self) -> Self::Seconds {
		self
	}
}
