
//! Autogenerated weights for `pallet_loans`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2024-03-04, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `runner`, CPU: `AMD EPYC 7763 64-Core Processor`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("centrifuge-dev"), DB CACHE: 1024

// Executed Command:
// target/release/centrifuge-chain
// benchmark
// pallet
// --chain=centrifuge-dev
// --steps=50
// --repeat=20
// --pallet=pallet_loans
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output=/tmp/runtime/centrifuge/src/weights/pallet_loans.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_loans`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_loans::WeightInfo for WeightInfo<T> {
	/// Storage: Permissions Permission (r:1 w:0)
	/// Proof: Permissions Permission (max_values: None, max_size: Some(228), added: 2703, mode: MaxEncodedLen)
	/// Storage: Uniques Asset (r:1 w:1)
	/// Proof: Uniques Asset (max_values: None, max_size: Some(138), added: 2613, mode: MaxEncodedLen)
	/// Storage: PoolSystem Pool (r:1 w:0)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: Uniques Class (r:1 w:0)
	/// Proof: Uniques Class (max_values: None, max_size: Some(182), added: 2657, mode: MaxEncodedLen)
	/// Storage: Loans LastLoanId (r:1 w:1)
	/// Proof: Loans LastLoanId (max_values: None, max_size: Some(32), added: 2507, mode: MaxEncodedLen)
	/// Storage: Loans CreatedLoan (r:0 w:1)
	/// Proof: Loans CreatedLoan (max_values: None, max_size: Some(244), added: 2719, mode: MaxEncodedLen)
	/// Storage: Uniques Account (r:0 w:2)
	/// Proof: Uniques Account (max_values: None, max_size: Some(104), added: 2579, mode: MaxEncodedLen)
	/// Storage: Uniques ItemPriceOf (r:0 w:1)
	/// Proof: Uniques ItemPriceOf (max_values: None, max_size: Some(105), added: 2580, mode: MaxEncodedLen)
	fn create() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1228`
		//  Estimated: `4278`
		// Minimum execution time: 83_065_000 picoseconds.
		Weight::from_parts(83_927_000, 0)
			.saturating_add(Weight::from_parts(0, 4278))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	/// Storage: Loans CreatedLoan (r:1 w:1)
	/// Proof: Loans CreatedLoan (max_values: None, max_size: Some(244), added: 2719, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:1)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: PoolSystem Pool (r:1 w:1)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: OrmlTokens Accounts (r:2 w:2)
	/// Proof: OrmlTokens Accounts (max_values: None, max_size: Some(129), added: 2604, mode: MaxEncodedLen)
	/// Storage: OrmlAssetRegistry Metadata (r:1 w:0)
	/// Proof Skipped: OrmlAssetRegistry Metadata (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn borrow(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `38281 + n * (340 ±0)`
		//  Estimated: `375491 + n * (340 ±0)`
		// Minimum execution time: 258_834_000 picoseconds.
		Weight::from_parts(270_918_292, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 52_483
			.saturating_add(Weight::from_parts(270_583, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(11))
			.saturating_add(T::DbWeight::get().writes(7))
			.saturating_add(Weight::from_parts(0, 340).saturating_mul(n.into()))
	}
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:0)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: PoolSystem Pool (r:1 w:1)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: OrmlTokens Accounts (r:2 w:2)
	/// Proof: OrmlTokens Accounts (max_values: None, max_size: Some(129), added: 2604, mode: MaxEncodedLen)
	/// Storage: OrmlAssetRegistry Metadata (r:1 w:0)
	/// Proof Skipped: OrmlAssetRegistry Metadata (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn repay(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `38434 + n * (340 ±0)`
		//  Estimated: `375491 + n * (340 ±0)`
		// Minimum execution time: 189_975_000 picoseconds.
		Weight::from_parts(194_674_487, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 44_229
			.saturating_add(Weight::from_parts(760_400, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(10))
			.saturating_add(T::DbWeight::get().writes(5))
			.saturating_add(Weight::from_parts(0, 340).saturating_mul(n.into()))
	}
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Loans WriteOffPolicy (r:1 w:0)
	/// Proof: Loans WriteOffPolicy (max_values: None, max_size: Some(5126), added: 7601, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:1)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn write_off(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `41141 + n * (340 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 361_806_000 picoseconds.
		Weight::from_parts(377_184_840, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 50_608
			.saturating_add(Weight::from_parts(385_565, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: Permissions Permission (r:1 w:0)
	/// Proof: Permissions Permission (max_values: None, max_size: Some(228), added: 2703, mode: MaxEncodedLen)
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Loans WriteOffPolicy (r:1 w:0)
	/// Proof: Loans WriteOffPolicy (max_values: None, max_size: Some(5126), added: 7601, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:1)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn admin_write_off(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `41392 + n * (340 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 375_490_000 picoseconds.
		Weight::from_parts(394_148_035, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 92_912
			.saturating_add(Weight::from_parts(490_645, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: Permissions Permission (r:1 w:0)
	/// Proof: Permissions Permission (max_values: None, max_size: Some(228), added: 2703, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:0)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: PoolSystem NotedChange (r:0 w:1)
	/// Proof: PoolSystem NotedChange (max_values: None, max_size: Some(6136), added: 8611, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn propose_loan_mutation(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `971 + n * (316 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 45_475_000 picoseconds.
		Weight::from_parts(47_440_607, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 11_156
			.saturating_add(Weight::from_parts(497_741, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: PoolSystem NotedChange (r:1 w:1)
	/// Proof: PoolSystem NotedChange (max_values: None, max_size: Some(6136), added: 8611, mode: MaxEncodedLen)
	/// Storage: PoolSystem Pool (r:1 w:0)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:0)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn apply_loan_mutation(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `37477 + n * (340 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 98_033_000 picoseconds.
		Weight::from_parts(103_175_207, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 28_174
			.saturating_add(Weight::from_parts(558_306, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: Loans CreatedLoan (r:1 w:0)
	/// Proof: Loans CreatedLoan (max_values: None, max_size: Some(244), added: 2719, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:1)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: Uniques Class (r:1 w:0)
	/// Proof: Uniques Class (max_values: None, max_size: Some(182), added: 2657, mode: MaxEncodedLen)
	/// Storage: Uniques Asset (r:1 w:1)
	/// Proof: Uniques Asset (max_values: None, max_size: Some(138), added: 2613, mode: MaxEncodedLen)
	/// Storage: Loans ClosedLoan (r:0 w:1)
	/// Proof: Loans ClosedLoan (max_values: None, max_size: Some(280), added: 2755, mode: MaxEncodedLen)
	/// Storage: Uniques Account (r:0 w:2)
	/// Proof: Uniques Account (max_values: None, max_size: Some(104), added: 2579, mode: MaxEncodedLen)
	/// Storage: Uniques ItemPriceOf (r:0 w:1)
	/// Proof: Uniques ItemPriceOf (max_values: None, max_size: Some(105), added: 2580, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn close(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `37337 + n * (373 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 143_118_000 picoseconds.
		Weight::from_parts(151_469_205, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 29_664
			.saturating_add(Weight::from_parts(781_522, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(8))
	}
	/// Storage: Permissions Permission (r:1 w:0)
	/// Proof: Permissions Permission (max_values: None, max_size: Some(228), added: 2703, mode: MaxEncodedLen)
	/// Storage: PoolSystem Pool (r:1 w:0)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: PoolSystem NotedChange (r:0 w:1)
	/// Proof: PoolSystem NotedChange (max_values: None, max_size: Some(6136), added: 8611, mode: MaxEncodedLen)
	fn propose_write_off_policy() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `478`
		//  Estimated: `4278`
		// Minimum execution time: 110_276_000 picoseconds.
		Weight::from_parts(111_718_000, 0)
			.saturating_add(Weight::from_parts(0, 4278))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: PoolSystem NotedChange (r:1 w:1)
	/// Proof: PoolSystem NotedChange (max_values: None, max_size: Some(6136), added: 8611, mode: MaxEncodedLen)
	/// Storage: PoolSystem Pool (r:1 w:0)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: Loans WriteOffPolicy (r:0 w:1)
	/// Proof: Loans WriteOffPolicy (max_values: None, max_size: Some(5126), added: 7601, mode: MaxEncodedLen)
	fn apply_write_off_policy() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4854`
		//  Estimated: `9601`
		// Minimum execution time: 118_902_000 picoseconds.
		Weight::from_parts(120_966_000, 0)
			.saturating_add(Weight::from_parts(0, 9601))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: PoolSystem Pool (r:1 w:0)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:0)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: OraclePriceCollection Collection (r:1 w:0)
	/// Proof: OraclePriceCollection Collection (max_values: None, max_size: Some(75042), added: 77517, mode: MaxEncodedLen)
	/// Storage: OraclePriceCollection CollectionInfo (r:1 w:0)
	/// Proof: OraclePriceCollection CollectionInfo (max_values: None, max_size: Some(6078), added: 8553, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:0)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: Loans PortfolioValuation (r:0 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 10]`.
	fn update_portfolio_valuation(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `37026 + n * (353 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 92_994_000 picoseconds.
		Weight::from_parts(85_363_227, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 16_880
			.saturating_add(Weight::from_parts(10_797_050, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Loans PortfolioValuation (r:1 w:0)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:0)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:0)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: Loans CreatedLoan (r:1 w:0)
	/// Proof: Loans CreatedLoan (max_values: None, max_size: Some(244), added: 2719, mode: MaxEncodedLen)
	/// Storage: PoolSystem NotedChange (r:0 w:1)
	/// Proof: PoolSystem NotedChange (max_values: None, max_size: Some(6136), added: 8611, mode: MaxEncodedLen)
	/// The range of component `n` is `[2, 8]`.
	fn propose_transfer_debt(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `37144 + n * (340 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 285_124_000 picoseconds.
		Weight::from_parts(292_535_387, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 72_345
			.saturating_add(Weight::from_parts(1_691_221, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: PoolSystem NotedChange (r:1 w:1)
	/// Proof: PoolSystem NotedChange (max_values: None, max_size: Some(6136), added: 8611, mode: MaxEncodedLen)
	/// Storage: PoolSystem Pool (r:1 w:0)
	/// Proof: PoolSystem Pool (max_values: None, max_size: Some(813), added: 3288, mode: MaxEncodedLen)
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:1)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: Loans CreatedLoan (r:1 w:1)
	/// Proof: Loans CreatedLoan (max_values: None, max_size: Some(244), added: 2719, mode: MaxEncodedLen)
	/// The range of component `n` is `[2, 8]`.
	fn apply_transfer_debt(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `37805 + n * (340 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 291_194_000 picoseconds.
		Weight::from_parts(300_874_911, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 81_720
			.saturating_add(Weight::from_parts(1_936_575, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	/// Storage: Loans CreatedLoan (r:1 w:1)
	/// Proof: Loans CreatedLoan (max_values: None, max_size: Some(244), added: 2719, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: InterestAccrual Rates (r:1 w:1)
	/// Proof: InterestAccrual Rates (max_values: Some(1), max_size: Some(36002), added: 36497, mode: MaxEncodedLen)
	/// Storage: InterestAccrual LastUpdated (r:1 w:0)
	/// Proof: InterestAccrual LastUpdated (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: Loans PortfolioValuation (r:1 w:1)
	/// Proof: Loans PortfolioValuation (max_values: None, max_size: Some(24050), added: 26525, mode: MaxEncodedLen)
	/// Storage: Loans ActiveLoans (r:1 w:1)
	/// Proof: Loans ActiveLoans (max_values: None, max_size: Some(372026), added: 374501, mode: MaxEncodedLen)
	/// The range of component `n` is `[1, 9]`.
	fn increase_debt(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `36804 + n * (340 ±0)`
		//  Estimated: `375491`
		// Minimum execution time: 186_598_000 picoseconds.
		Weight::from_parts(196_558_588, 0)
			.saturating_add(Weight::from_parts(0, 375491))
			// Standard Error: 39_911
			.saturating_add(Weight::from_parts(810_756, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(4))
	}
}
