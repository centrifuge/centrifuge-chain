
//! Autogenerated weights for `pallet_keystore`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2025-02-24, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `runner`, CPU: `AMD EPYC 7763 64-Core Processor`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("centrifuge-local")`, DB CACHE: 1024

// Executed Command:
// target/release/centrifuge-chain
// benchmark
// pallet
// --chain=centrifuge-local
// --steps=50
// --repeat=20
// --pallet=pallet_keystore
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --output=/tmp/runtime/centrifuge/src/weights/pallet_keystore.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_keystore`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_keystore::WeightInfo for WeightInfo<T> {
	/// Storage: `Keystore::KeyDeposit` (r:1 w:0)
	/// Proof: `Keystore::KeyDeposit` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Keystore::Keys` (r:10 w:10)
	/// Proof: `Keystore::Keys` (`max_values`: None, `max_size`: Some(120), added: 2595, mode: `MaxEncodedLen`)
	/// Storage: `Keystore::LastKeyByPurpose` (r:0 w:1)
	/// Proof: `Keystore::LastKeyByPurpose` (`max_values`: None, `max_size`: Some(97), added: 2572, mode: `MaxEncodedLen`)
	/// The range of component `n` is `[1, 10]`.
	fn add_keys(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `250`
		//  Estimated: `3593 + n * (2595 ±0)`
		// Minimum execution time: 37_040_000 picoseconds.
		Weight::from_parts(16_186_258, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			// Standard Error: 32_026
			.saturating_add(Weight::from_parts(22_061_296, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(n.into())))
			.saturating_add(T::DbWeight::get().writes(2))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(n.into())))
			.saturating_add(Weight::from_parts(0, 2595).saturating_mul(n.into()))
	}
	/// Storage: `Keystore::Keys` (r:10 w:10)
	/// Proof: `Keystore::Keys` (`max_values`: None, `max_size`: Some(120), added: 2595, mode: `MaxEncodedLen`)
	/// The range of component `n` is `[1, 10]`.
	fn revoke_keys(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `148 + n * (75 ±0)`
		//  Estimated: `990 + n * (2595 ±0)`
		// Minimum execution time: 17_082_000 picoseconds.
		Weight::from_parts(8_106_640, 0)
			.saturating_add(Weight::from_parts(0, 990))
			// Standard Error: 18_739
			.saturating_add(Weight::from_parts(9_924_968, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(n.into())))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(n.into())))
			.saturating_add(Weight::from_parts(0, 2595).saturating_mul(n.into()))
	}
	/// Storage: `Keystore::KeyDeposit` (r:0 w:1)
	/// Proof: `Keystore::KeyDeposit` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	fn set_deposit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 6_913_000 picoseconds.
		Weight::from_parts(7_164_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
