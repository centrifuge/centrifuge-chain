//! A set of constant values used in altair runtime

/// Money matters.
pub mod currency {
	use cfg_primitives::{
		constants::{CENTI_CFG, CFG, MICRO_CFG, MILLI_CFG},
		types::Balance,
	};

	pub const MICRO_AIR: Balance = MICRO_CFG;
	pub const MILLI_AIR: Balance = MILLI_CFG;
	pub const CENTI_AIR: Balance = CENTI_CFG;
	pub const AIR: Balance = CFG;
}
