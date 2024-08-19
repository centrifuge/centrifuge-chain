use cfg_primitives::{Days, Millis, Seconds};
use frame_support::traits::UnixTime;

/// Trait to obtain the unix time as seconds
pub trait UnixTimeSecs: UnixTime {
	fn now() -> Seconds {
		Seconds::from(<Self as UnixTime>::now().as_secs())
	}

	/// Same as now(), shortcut for cases where `now()` conflicts with
	/// `UnixTime::now()`
	fn now_secs() -> Seconds {
		<Self as UnixTimeSecs>::now()
	}
}

impl<T: UnixTime> UnixTimeSecs for T {}

/// Trait to handle an unknown time unit type
pub trait TimeUnit {
	fn as_millis(self) -> Millis;
	fn as_seconds(self) -> Seconds;
	fn as_days(self) -> Days;
}

impl TimeUnit for Millis {
	fn as_millis(self) -> Millis {
		self
	}

	fn as_seconds(self) -> Seconds {
		self.into_seconds()
	}

	fn as_days(self) -> Days {
		self.into_days()
	}
}

impl TimeUnit for Seconds {
	fn as_millis(self) -> Millis {
		self.into_millis()
	}

	fn as_seconds(self) -> Seconds {
		self
	}

	fn as_days(self) -> Days {
		self.into_days()
	}
}

impl TimeUnit for Days {
	fn as_millis(self) -> Millis {
		self.into_millis()
	}

	fn as_seconds(self) -> Seconds {
		self.into_seconds()
	}

	fn as_days(self) -> Days {
		self
	}
}
