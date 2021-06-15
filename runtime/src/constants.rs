//! A set of constant values used in altair runtime

/// Money matters.
pub mod currency {
    use node_primitives::Balance;

    pub const MICRO_AIR: Balance = 1_000_000_000_000; // 10−6 	0.000001
    pub const MILLI_AIR: Balance = 1_000 * MICRO_AIR; // 10−3 	0.001
    pub const CENTI_AIR: Balance = 10 * MILLI_AIR; // 10−2 	0.01
    pub const AIR: Balance = 100 * CENTI_AIR;

    pub const fn deposit(items: u32, bytes: u32) -> Balance {
        items as Balance * 15 * CENTI_AIR + (bytes as Balance) * 6 * CENTI_AIR
    }

}
