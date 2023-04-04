#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_crowdloan_reward`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_elections_phragmen::WeightInfo for WeightInfo<T> {
    fn vote_equal(v: u32, ) -> Weight {
        Weight::from_ref_time(27_011_000 as u64)
            // Standard Error: 3_000
            .saturating_add(Weight::from_ref_time(214_000 as u64).saturating_mul(v as u64))
            .saturating_add(T::DbWeight::get().reads(5 as u64))
            .saturating_add(T::DbWeight::get().writes(2 as u64))
    }
    // Storage: Elections Candidates (r:1 w:0)
    // Storage: Elections Members (r:1 w:0)
    // Storage: Elections RunnersUp (r:1 w:0)
    // Storage: Elections Voting (r:1 w:1)
    // Storage: Balances Locks (r:1 w:1)
    /// The range of component `v` is `[2, 16]`.
    fn vote_more(v: u32, ) -> Weight {
        Weight::from_ref_time(40_240_000 as u64)
            // Standard Error: 5_000
            .saturating_add(Weight::from_ref_time(244_000 as u64).saturating_mul(v as u64))
            .saturating_add(T::DbWeight::get().reads(5 as u64))
            .saturating_add(T::DbWeight::get().writes(2 as u64))
    }
    // Storage: Elections Candidates (r:1 w:0)
    // Storage: Elections Members (r:1 w:0)
    // Storage: Elections RunnersUp (r:1 w:0)
    // Storage: Elections Voting (r:1 w:1)
    // Storage: Balances Locks (r:1 w:1)
    /// The range of component `v` is `[2, 16]`.
    fn vote_less(v: u32, ) -> Weight {
        Weight::from_ref_time(40_394_000 as u64)
            // Standard Error: 5_000
            .saturating_add(Weight::from_ref_time(217_000 as u64).saturating_mul(v as u64))
            .saturating_add(T::DbWeight::get().reads(5 as u64))
            .saturating_add(T::DbWeight::get().writes(2 as u64))
    }
    // Storage: Elections Voting (r:1 w:1)
    // Storage: Balances Locks (r:1 w:1)
    fn remove_voter() -> Weight {
        Weight::from_ref_time(37_651_000 as u64)
            .saturating_add(T::DbWeight::get().reads(2 as u64))
            .saturating_add(T::DbWeight::get().writes(2 as u64))
    }
    // Storage: Elections Candidates (r:1 w:1)
    // Storage: Elections Members (r:1 w:0)
    // Storage: Elections RunnersUp (r:1 w:0)
    /// The range of component `c` is `[1, 1000]`.
    fn submit_candidacy(c: u32, ) -> Weight {
        Weight::from_ref_time(42_217_000 as u64)
            // Standard Error: 0
            .saturating_add(Weight::from_ref_time(50_000 as u64).saturating_mul(c as u64))
            .saturating_add(T::DbWeight::get().reads(3 as u64))
            .saturating_add(T::DbWeight::get().writes(1 as u64))
    }
    // Storage: Elections Candidates (r:1 w:1)
    /// The range of component `c` is `[1, 1000]`.
    fn renounce_candidacy_candidate(c: u32, ) -> Weight {
        Weight::from_ref_time(46_459_000 as u64)
            // Standard Error: 0
            .saturating_add(Weight::from_ref_time(26_000 as u64).saturating_mul(c as u64))
            .saturating_add(T::DbWeight::get().reads(1 as u64))
            .saturating_add(T::DbWeight::get().writes(1 as u64))
    }
    // Storage: Elections Members (r:1 w:1)
    // Storage: Elections RunnersUp (r:1 w:1)
    // Storage: Council Prime (r:1 w:1)
    // Storage: Council Proposals (r:1 w:0)
    // Storage: Council Members (r:0 w:1)
    fn renounce_candidacy_members() -> Weight {
        Weight::from_ref_time(45_189_000 as u64)
            .saturating_add(T::DbWeight::get().reads(4 as u64))
            .saturating_add(T::DbWeight::get().writes(4 as u64))
    }
    // Storage: Elections RunnersUp (r:1 w:1)
    fn renounce_candidacy_runners_up() -> Weight {
        Weight::from_ref_time(34_516_000 as u64)
            .saturating_add(T::DbWeight::get().reads(1 as u64))
            .saturating_add(T::DbWeight::get().writes(1 as u64))
    }
    // Storage: Benchmark Override (r:0 w:0)
    fn remove_member_without_replacement() -> Weight {
        Weight::from_ref_time(2_000_000_000_000 as u64)
    }
    // Storage: Elections Members (r:1 w:1)
    // Storage: System Account (r:1 w:1)
    // Storage: Elections RunnersUp (r:1 w:1)
    // Storage: Council Prime (r:1 w:1)
    // Storage: Council Proposals (r:1 w:0)
    // Storage: Council Members (r:0 w:1)
    fn remove_member_with_replacement() -> Weight {
        Weight::from_ref_time(51_838_000 as u64)
            .saturating_add(T::DbWeight::get().reads(5 as u64))
            .saturating_add(T::DbWeight::get().writes(5 as u64))
    }
    // Storage: Elections Voting (r:5001 w:5000)
    // Storage: Elections Members (r:1 w:0)
    // Storage: Elections RunnersUp (r:1 w:0)
    // Storage: Elections Candidates (r:1 w:0)
    // Storage: Balances Locks (r:5000 w:5000)
    // Storage: System Account (r:5000 w:5000)
    /// The range of component `v` is `[5000, 10000]`.
    /// The range of component `d` is `[1, 5000]`.
    fn clean_defunct_voters(v: u32, _d: u32, ) -> Weight {
        Weight::from_ref_time(0 as u64)
            // Standard Error: 76_000
            .saturating_add(Weight::from_ref_time(63_721_000 as u64).saturating_mul(v as u64))
            .saturating_add(T::DbWeight::get().reads(4 as u64))
            .saturating_add(T::DbWeight::get().reads((3 as u64).saturating_mul(v as u64)))
            .saturating_add(T::DbWeight::get().writes((3 as u64).saturating_mul(v as u64)))
    }
    // Storage: Elections Candidates (r:1 w:1)
    // Storage: Elections Members (r:1 w:1)
    // Storage: Elections RunnersUp (r:1 w:1)
    // Storage: Elections Voting (r:10001 w:0)
    // Storage: Council Proposals (r:1 w:0)
    // Storage: Elections ElectionRounds (r:1 w:1)
    // Storage: Council Members (r:0 w:1)
    // Storage: Council Prime (r:0 w:1)
    // Storage: System Account (r:1 w:1)
    /// The range of component `c` is `[1, 1000]`.
    /// The range of component `v` is `[1, 10000]`.
    /// The range of component `e` is `[10000, 160000]`.
    fn election_phragmen(c: u32, v: u32, e: u32, ) -> Weight {
        Weight::from_ref_time(0 as u64)
            // Standard Error: 773_000
            .saturating_add(Weight::from_ref_time(81_534_000 as u64).saturating_mul(v as u64))
            // Standard Error: 51_000
            .saturating_add(Weight::from_ref_time(4_453_000 as u64).saturating_mul(e as u64))
            .saturating_add(T::DbWeight::get().reads(280 as u64))
            .saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(c as u64)))
            .saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(v as u64)))
            .saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(c as u64)))
    }
}
