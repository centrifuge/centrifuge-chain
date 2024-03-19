use cfg_primitives::{
	constants::{CENTI_CFG, TREASURY_FEE_RATIO},
	types::Balance,
	AccountId,
};
use cfg_traits::fees::{Fee, Fees, PayFee};
use cfg_types::fee_keys::FeeKey;
use frame_support::{
	dispatch::DispatchResult,
	traits::{Currency, Get, Imbalance, OnUnbalanced},
	weights::{
		constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients,
		WeightToFeePolynomial,
	},
};
use smallvec::smallvec;
use sp_arithmetic::Perbill;

pub type NegativeImbalance<R> = <pallet_balances::Pallet<R> as Currency<
	<R as frame_system::Config>::AccountId,
>>::NegativeImbalance;

struct ToAuthor<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for ToAuthor<R>
where
	R: pallet_balances::Config + pallet_authorship::Config,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
		if let Some(author) = <pallet_authorship::Pallet<R>>::author() {
			<pallet_balances::Pallet<R>>::resolve_creating(&author, amount);
		}
	}
}

pub struct DealWithFees<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_treasury::Config + pallet_authorship::Config,
	pallet_treasury::Pallet<R>: OnUnbalanced<NegativeImbalance<R>>,
{
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
		if let Some(fees) = fees_then_tips.next() {
			// for fees, split the destination
			let (treasury_amount, mut author_amount) = fees.ration(
				TREASURY_FEE_RATIO.deconstruct(),
				(Perbill::one() - TREASURY_FEE_RATIO).deconstruct(),
			);
			if let Some(tips) = fees_then_tips.next() {
				// for tips, if any, 100% to author
				tips.merge_into(&mut author_amount);
			}

			use pallet_treasury::Pallet as Treasury;
			<Treasury<R> as OnUnbalanced<_>>::on_unbalanced(treasury_amount);
			<ToAuthor<R> as OnUnbalanced<_>>::on_unbalanced(author_amount);
		}
	}
}

/// Handles converting a weight scalar to a fee value, based on the scale
/// and granularity of the node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - [0, frame_system::MaximumBlockWeight]
///   - [Balance::min, Balance::max]
///
/// Yet, it can be used for any other sort of change to weight-fee. Some
/// examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be
///     charged.
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;

	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		let p = CENTI_CFG;
		let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time());

		smallvec!(WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q,
		})
	}
}

/// See doc from PayFee
pub struct FeeToTreasury<F, V>(sp_std::marker::PhantomData<(F, V)>);
impl<
		F: Fees<AccountId = AccountId, Balance = Balance, FeeKey = FeeKey>,
		V: Get<Fee<Balance, FeeKey>>,
	> PayFee<AccountId> for FeeToTreasury<F, V>
{
	fn pay(payer: &AccountId) -> DispatchResult {
		F::fee_to_treasury(payer, V::get())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn add_pay_requirements(payer: &AccountId) {
		F::add_fee_requirements(payer, V::get());
	}
}

/// See doc from PayFee
pub struct FeeToAuthor<F, V>(sp_std::marker::PhantomData<(F, V)>);
impl<
		F: Fees<AccountId = AccountId, Balance = Balance, FeeKey = FeeKey>,
		V: Get<Fee<Balance, FeeKey>>,
	> PayFee<AccountId> for FeeToAuthor<F, V>
{
	fn pay(payer: &AccountId) -> DispatchResult {
		F::fee_to_author(payer, V::get())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn add_pay_requirements(payer: &AccountId) {
		F::add_fee_requirements(payer, V::get());
	}
}

/// See doc from PayFee
pub struct FeeToBurn<F, V>(sp_std::marker::PhantomData<(F, V)>);
impl<
		F: Fees<AccountId = AccountId, Balance = Balance, FeeKey = FeeKey>,
		V: Get<Fee<Balance, FeeKey>>,
	> PayFee<AccountId> for FeeToBurn<F, V>
{
	fn pay(payer: &AccountId) -> DispatchResult {
		F::fee_to_burn(payer, V::get())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn add_pay_requirements(payer: &AccountId) {
		F::add_fee_requirements(payer, V::get());
	}
}

#[cfg(test)]
mod test {
	use cfg_primitives::{AccountId, TREASURY_FEE_RATIO};
	use cfg_types::ids::TREASURY_PALLET_ID;
	use frame_support::{
		derive_impl, parameter_types,
		traits::{Currency, FindAuthor},
		PalletId,
	};
	use sp_core::ConstU64;
	use sp_runtime::{traits::IdentityLookup, Perbill};
	use sp_std::convert::{TryFrom, TryInto};

	use super::*;

	const TEST_ACCOUNT: AccountId = AccountId::new([1; 32]);

	frame_support::construct_runtime!(
		pub enum Runtime {
			System: frame_system,
			Authorship: pallet_authorship,
			Balances: pallet_balances,
			Treasury: pallet_treasury,
		}
	);

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
	}

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
	impl frame_system::Config for Runtime {
		type AccountData = pallet_balances::AccountData<u64>;
		type AccountId = AccountId;
		type Block = frame_system::mocking::MockBlock<Runtime>;
		type Lookup = IdentityLookup<Self::AccountId>;
	}

	#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
	impl pallet_balances::Config for Runtime {
		type AccountStore = System;
		type DustRemoval = ();
		type ExistentialDeposit = ConstU64<1>;
		type RuntimeHoldReason = ();
	}

	parameter_types! {
		pub const TreasuryPalletId: PalletId = TREASURY_PALLET_ID;
		pub const MaxApprovals: u32 = 100;
	}

	impl pallet_treasury::Config for Runtime {
		type ApproveOrigin = frame_system::EnsureRoot<AccountId>;
		type Burn = ();
		type BurnDestination = ();
		type Currency = pallet_balances::Pallet<Runtime>;
		type MaxApprovals = MaxApprovals;
		type OnSlash = ();
		type PalletId = TreasuryPalletId;
		type ProposalBond = ();
		type ProposalBondMaximum = ();
		type ProposalBondMinimum = ();
		type RejectOrigin = frame_system::EnsureRoot<AccountId>;
		type RuntimeEvent = RuntimeEvent;
		type SpendFunds = ();
		type SpendOrigin = frame_support::traits::NeverEnsureOrigin<u64>;
		type SpendPeriod = ();
		type WeightInfo = ();
	}

	pub struct OneAuthor;
	impl FindAuthor<AccountId> for OneAuthor {
		fn find_author<'a, I>(_: I) -> Option<AccountId>
		where
			I: 'a,
		{
			Some(TEST_ACCOUNT)
		}
	}
	impl pallet_authorship::Config for Runtime {
		type EventHandler = ();
		type FindAuthor = OneAuthor;
	}

	#[test]
	fn test_fees_and_tip_split() {
		System::externalities().execute_with(|| {
			const FEE: u64 = 10;
			const TIP: u64 = 20;

			let fee = Balances::issue(FEE);
			let tip = Balances::issue(TIP);

			assert_eq!(Balances::free_balance(Treasury::account_id()), 0);
			assert_eq!(Balances::free_balance(TEST_ACCOUNT), 0);

			DealWithFees::on_unbalanceds(vec![fee, tip].into_iter());

			assert_eq!(
				Balances::free_balance(Treasury::account_id()),
				TREASURY_FEE_RATIO * FEE
			);
			assert_eq!(
				Balances::free_balance(TEST_ACCOUNT),
				TIP + (Perbill::one() - TREASURY_FEE_RATIO) * FEE
			);
		});
	}
}
