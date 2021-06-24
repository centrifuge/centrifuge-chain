//! A set of constant values used in altair runtime

/// Money matters.
pub mod currency {
	use node_primitives::Balance;
	use runtime_common::*;

	pub const MICRO_AIR: Balance = MICRO_CFG;
	pub const MILLI_AIR: Balance = MILLI_CFG;
	pub const CENTI_AIR: Balance = CENTI_CFG;
	pub const AIR: Balance = CFG;
}
