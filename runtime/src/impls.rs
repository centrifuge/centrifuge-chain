//! Some configurable implementations as associated type for the substrate runtime.
use crate::{Authorship, Balances, MaximumBlockWeight, NegativeImbalance, System};
use frame_support::{
    traits::{Currency, Get, OnUnbalanced},
    weights::Weight,
};
use node_primitives::Balance;
use sp_runtime::traits::{Convert, Saturating};
use sp_runtime::{Fixed64, Perbill};

pub struct Author;
impl OnUnbalanced<NegativeImbalance> for Author {
    fn on_nonzero_unbalanced(amount: NegativeImbalance) {
        Balances::resolve_creating(&Authorship::author(), amount);
    }
}

/// Struct that handles the conversion of Balance -> `u64`. This is used for staking's election
/// calculation.
pub struct CurrencyToVoteHandler;

impl CurrencyToVoteHandler {
    fn factor() -> Balance {
        (Balances::total_issuance() / u64::max_value() as Balance).max(1)
    }
}

impl Convert<Balance, u64> for CurrencyToVoteHandler {
    fn convert(x: Balance) -> u64 {
        (x / Self::factor()) as u64
    }
}

impl Convert<u128, Balance> for CurrencyToVoteHandler {
    fn convert(x: u128) -> Balance {
        x * Self::factor()
    }
}

/// Convert from weight to balance via a simple coefficient multiplication
/// The associated type C encapsulates a constant in units of balance per weight
pub struct LinearWeightToFee<C>(sp_std::marker::PhantomData<C>);

impl<C: Get<Balance>> Convert<Weight, Balance> for LinearWeightToFee<C> {
    fn convert(w: Weight) -> Balance {
        // substrate-node a weight of 10_000 (smallest non-zero weight) to be mapped to 10^7 units of
        // fees, hence:
        let coefficient = C::get();
        Balance::from(w).saturating_mul(coefficient)
    }
}

/// Update the given multiplier based on the following formula
///
///   diff = (previous_block_weight - target_weight)
///   v = 0.00004
///   next_weight = weight * (1 + (v . diff) + (v . diff)^2 / 2)
///
/// Where `target_weight` must be given as the `Get` implementation of the `T` generic type.
/// https://research.web3.foundation/en/latest/polkadot/Token%20Economics/#relay-chain-transaction-fees
pub struct TargetedFeeAdjustment<T>(sp_std::marker::PhantomData<T>);

impl<T: Get<Perbill>> Convert<Fixed64, Fixed64> for TargetedFeeAdjustment<T> {
    fn convert(multiplier: Fixed64) -> Fixed64 {
        let block_weight = System::all_extrinsics_weight();
        let max_weight = MaximumBlockWeight::get();
        let target_weight = (T::get() * max_weight) as u128;
        let block_weight = block_weight as u128;

        // determines if the first_term is positive
        let positive = block_weight >= target_weight;
        let diff_abs = block_weight.max(target_weight) - block_weight.min(target_weight);
        // diff is within u32, safe.
        let diff = Fixed64::from_rational(diff_abs as i64, max_weight as u64);
        let diff_squared = diff.saturating_mul(diff);

        // 0.00004 = 4/100_000 = 40_000/10^9
        let v = Fixed64::from_rational(4, 100_000);
        // 0.00004^2 = 16/10^10 ~= 2/10^9. Taking the future /2 into account, then it is just 1
        // parts from a billionth.
        let v_squared_2 = Fixed64::from_rational(1, 1_000_000_000);

        let first_term = v.saturating_mul(diff);
        // It is very unlikely that this will exist (in our poor perbill estimate) but we are giving
        // it a shot.
        let second_term = v_squared_2.saturating_mul(diff_squared);

        if positive {
            // Note: this is merely bounded by how big the multiplier and the inner value can go,
            // not by any economical reasoning.
            let excess = first_term.saturating_add(second_term);
            multiplier.saturating_add(excess)
        } else {
            // Proof: first_term > second_term. Safe subtraction.
            let negative = first_term - second_term;
            multiplier
                .saturating_sub(negative)
                // despite the fact that apply_to saturates weight (final fee cannot go below 0)
                // it is crucially important to stop here and don't further reduce the weight fee
                // multiplier. While at -1, it means that the network is so un-congested that all
                // transactions have no weight fee. We stop here and only increase if the network
                // became more busy.
                .max(Fixed64::from_rational(-1, 1))
        }
    }
}
