
//! Autogenerated weights for `pallet_order_book`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2025-02-24, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `runner`, CPU: `AMD EPYC 7763 64-Core Processor`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("development")`, DB CACHE: 1024

// Executed Command:
// target/release/centrifuge-chain
// benchmark
// pallet
// --chain=development
// --steps=50
// --repeat=20
// --pallet=pallet_order_book
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --output=/tmp/runtime/development/src/weights/pallet_order_book.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_order_book`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_order_book::WeightInfo for WeightInfo<T> {
	/// Storage: `OrmlAssetRegistry::Metadata` (r:1 w:0)
	/// Proof: `OrmlAssetRegistry::Metadata` (`max_values`: None, `max_size`: Some(942), added: 3417, mode: `MaxEncodedLen`)
	/// Storage: `OrderBook::OrderIdNonceStore` (r:1 w:1)
	/// Proof: `OrderBook::OrderIdNonceStore` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `OrmlTokens::Accounts` (r:1 w:1)
	/// Proof: `OrmlTokens::Accounts` (`max_values`: None, `max_size`: Some(129), added: 2604, mode: `MaxEncodedLen`)
	/// Storage: `OrderBook::Orders` (r:0 w:1)
	/// Proof: `OrderBook::Orders` (`max_values`: None, `max_size`: Some(171), added: 2646, mode: `MaxEncodedLen`)
	/// Storage: `OrderBook::UserOrders` (r:0 w:1)
	/// Proof: `OrderBook::UserOrders` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	fn place_order() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1163`
		//  Estimated: `4407`
		// Minimum execution time: 49_854_000 picoseconds.
		Weight::from_parts(52_108_000, 0)
			.saturating_add(Weight::from_parts(0, 4407))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: `OrderBook::Orders` (r:1 w:1)
	/// Proof: `OrderBook::Orders` (`max_values`: None, `max_size`: Some(171), added: 2646, mode: `MaxEncodedLen`)
	/// Storage: `OrmlAssetRegistry::Metadata` (r:1 w:0)
	/// Proof: `OrmlAssetRegistry::Metadata` (`max_values`: None, `max_size`: Some(942), added: 3417, mode: `MaxEncodedLen`)
	/// Storage: `OrmlTokens::Accounts` (r:1 w:1)
	/// Proof: `OrmlTokens::Accounts` (`max_values`: None, `max_size`: Some(129), added: 2604, mode: `MaxEncodedLen`)
	fn update_order() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1367`
		//  Estimated: `4407`
		// Minimum execution time: 45_856_000 picoseconds.
		Weight::from_parts(47_178_000, 0)
			.saturating_add(Weight::from_parts(0, 4407))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `OrderBook::Orders` (r:1 w:1)
	/// Proof: `OrderBook::Orders` (`max_values`: None, `max_size`: Some(171), added: 2646, mode: `MaxEncodedLen`)
	/// Storage: `OrmlTokens::Accounts` (r:1 w:1)
	/// Proof: `OrmlTokens::Accounts` (`max_values`: None, `max_size`: Some(129), added: 2604, mode: `MaxEncodedLen`)
	/// Storage: `OrmlAssetRegistry::Metadata` (r:1 w:0)
	/// Proof: `OrmlAssetRegistry::Metadata` (`max_values`: None, `max_size`: Some(942), added: 3417, mode: `MaxEncodedLen`)
	/// Storage: `OrderBook::UserOrders` (r:0 w:1)
	/// Proof: `OrderBook::UserOrders` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	fn cancel_order() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1367`
		//  Estimated: `4407`
		// Minimum execution time: 48_791_000 picoseconds.
		Weight::from_parts(51_055_000, 0)
			.saturating_add(Weight::from_parts(0, 4407))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `OrderBook::Orders` (r:1 w:1)
	/// Proof: `OrderBook::Orders` (`max_values`: None, `max_size`: Some(171), added: 2646, mode: `MaxEncodedLen`)
	/// Storage: `OrmlAssetRegistry::Metadata` (r:2 w:0)
	/// Proof: `OrmlAssetRegistry::Metadata` (`max_values`: None, `max_size`: Some(942), added: 3417, mode: `MaxEncodedLen`)
	/// Storage: `OrderBook::MarketFeederId` (r:1 w:0)
	/// Proof: `OrderBook::MarketFeederId` (`max_values`: Some(1), `max_size`: Some(604), added: 1099, mode: `MaxEncodedLen`)
	/// Storage: `OraclePriceFeed::FedValues` (r:1 w:0)
	/// Proof: `OraclePriceFeed::FedValues` (`max_values`: None, `max_size`: Some(711), added: 3186, mode: `MaxEncodedLen`)
	/// Storage: `OrmlTokens::Accounts` (r:4 w:4)
	/// Proof: `OrmlTokens::Accounts` (`max_values`: None, `max_size`: Some(129), added: 2604, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `ForeignInvestments::OrderIdToSwapId` (r:1 w:0)
	/// Proof: `ForeignInvestments::OrderIdToSwapId` (`max_values`: None, `max_size`: Some(81), added: 2556, mode: `MaxEncodedLen`)
	/// Storage: `OrderBook::UserOrders` (r:0 w:1)
	/// Proof: `OrderBook::UserOrders` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	fn fill_order() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1933`
		//  Estimated: `11406`
		// Minimum execution time: 144_911_000 picoseconds.
		Weight::from_parts(147_145_000, 0)
			.saturating_add(Weight::from_parts(0, 11406))
			.saturating_add(T::DbWeight::get().reads(12))
			.saturating_add(T::DbWeight::get().writes(8))
	}
	/// Storage: `OrderBook::MarketFeederId` (r:0 w:1)
	/// Proof: `OrderBook::MarketFeederId` (`max_values`: Some(1), `max_size`: Some(604), added: 1099, mode: `MaxEncodedLen`)
	fn set_market_feeder() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_115_000 picoseconds.
		Weight::from_parts(8_365_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
