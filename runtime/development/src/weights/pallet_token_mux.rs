
//! Autogenerated weights for `pallet_token_mux`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2024-03-04, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `runner`, CPU: `AMD EPYC 7763 64-Core Processor`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("development-local"), DB CACHE: 1024

// Executed Command:
// target/release/centrifuge-chain
// benchmark
// pallet
// --chain=development-local
// --steps=50
// --repeat=20
// --pallet=pallet_token_mux
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output=/tmp/runtime/development/src/weights/pallet_token_mux.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_token_mux`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_token_mux::WeightInfo for WeightInfo<T> {
	/// Storage: OrmlAssetRegistry Metadata (r:2 w:0)
	/// Proof Skipped: OrmlAssetRegistry Metadata (max_values: None, max_size: None, mode: Measured)
	/// Storage: OrmlTokens Accounts (r:3 w:3)
	/// Proof: OrmlTokens Accounts (max_values: None, max_size: Some(129), added: 2604, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: OrmlTokens TotalIssuance (r:1 w:1)
	/// Proof: OrmlTokens TotalIssuance (max_values: None, max_size: Some(49), added: 2524, mode: MaxEncodedLen)
	fn deposit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1357`
		//  Estimated: `8802`
		// Minimum execution time: 126_054_000 picoseconds.
		Weight::from_parts(127_257_000, 0)
			.saturating_add(Weight::from_parts(0, 8802))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	/// Storage: OrmlAssetRegistry Metadata (r:2 w:0)
	/// Proof Skipped: OrmlAssetRegistry Metadata (max_values: None, max_size: None, mode: Measured)
	/// Storage: OrmlTokens Accounts (r:3 w:3)
	/// Proof: OrmlTokens Accounts (max_values: None, max_size: Some(129), added: 2604, mode: MaxEncodedLen)
	/// Storage: OrmlTokens TotalIssuance (r:1 w:1)
	/// Proof: OrmlTokens TotalIssuance (max_values: None, max_size: Some(49), added: 2524, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn burn() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1567`
		//  Estimated: `8802`
		// Minimum execution time: 111_337_000 picoseconds.
		Weight::from_parts(113_000_000, 0)
			.saturating_add(Weight::from_parts(0, 8802))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: OrderBook Orders (r:1 w:1)
	/// Proof: OrderBook Orders (max_values: None, max_size: Some(171), added: 2646, mode: MaxEncodedLen)
	/// Storage: OrmlAssetRegistry Metadata (r:2 w:0)
	/// Proof Skipped: OrmlAssetRegistry Metadata (max_values: None, max_size: None, mode: Measured)
	/// Storage: OrmlTokens Accounts (r:4 w:4)
	/// Proof: OrmlTokens Accounts (max_values: None, max_size: Some(129), added: 2604, mode: MaxEncodedLen)
	/// Storage: OrmlTokens TotalIssuance (r:1 w:1)
	/// Proof: OrmlTokens TotalIssuance (max_values: None, max_size: Some(49), added: 2524, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Swaps OrderIdToSwapId (r:1 w:0)
	/// Proof: Swaps OrderIdToSwapId (max_values: None, max_size: Some(81), added: 2556, mode: MaxEncodedLen)
	/// Storage: OrderBook UserOrders (r:0 w:1)
	/// Proof: OrderBook UserOrders (max_values: None, max_size: Some(56), added: 2531, mode: MaxEncodedLen)
	fn match_swap() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1993`
		//  Estimated: `11406`
		// Minimum execution time: 221_883_000 picoseconds.
		Weight::from_parts(223_416_000, 0)
			.saturating_add(Weight::from_parts(0, 11406))
			.saturating_add(T::DbWeight::get().reads(11))
			.saturating_add(T::DbWeight::get().writes(9))
	}
}