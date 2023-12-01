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

pub struct FeeToTreasury<F, V>(sp_std::marker::PhantomData<(F, V)>);
impl<
		F: Fees<AccountId = AccountId, Balance = Balance, FeeKey = FeeKey>,
		V: Get<Fee<Balance, FeeKey>>,
	> PayFee<AccountId> for FeeToTreasury<F, V>
{
	fn pay(who: &AccountId) -> DispatchResult {
		F::fee_to_treasury(who, V::get())
	}
}

pub struct FeeToAuthor<F, V>(sp_std::marker::PhantomData<(F, V)>);
impl<
		F: Fees<AccountId = AccountId, Balance = Balance, FeeKey = FeeKey>,
		V: Get<Fee<Balance, FeeKey>>,
	> PayFee<AccountId> for FeeToAuthor<F, V>
{
	fn pay(who: &AccountId) -> DispatchResult {
		F::fee_to_author(who, V::get())
	}
}

pub struct FeeToBurn<F, V>(sp_std::marker::PhantomData<(F, V)>);
impl<
		F: Fees<AccountId = AccountId, Balance = Balance, FeeKey = FeeKey>,
		V: Get<Fee<Balance, FeeKey>>,
	> PayFee<AccountId> for FeeToBurn<F, V>
{
	fn pay(who: &AccountId) -> DispatchResult {
		F::fee_to_burn(who, V::get())
	}
}

#[cfg(test)]
mod test {
	use cfg_primitives::{AccountId, TREASURY_FEE_RATIO};
	use frame_support::{
		parameter_types,
		traits::{Currency, FindAuthor},
		PalletId,
	};
	use sp_core::{ConstU64, H256};
	use sp_io::TestExternalities;
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
		Perbill,
	};
	use sp_std::convert::{TryFrom, TryInto};

	use super::*;

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
	type Block = frame_system::mocking::MockBlock<Runtime>;

	const TEST_ACCOUNT: AccountId = AccountId::new([1; 32]);

	frame_support::construct_runtime!(
		pub enum Runtime where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			Authorship: pallet_authorship::{Pallet, Storage},
			Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
			Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>},
		}
	);

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
	}

	impl frame_system::Config for Runtime {
		type AccountData = pallet_balances::AccountData<u64>;
		type AccountId = AccountId;
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockHashCount = BlockHashCount;
		type BlockLength = ();
		type BlockNumber = u64;
		type BlockWeights = ();
		type DbWeight = ();
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Header = Header;
		type Index = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type MaxConsumers = frame_support::traits::ConstU32<16>;
		type OnKilledAccount = ();
		type OnNewAccount = ();
		type OnSetCode = ();
		type PalletInfo = PalletInfo;
		type RuntimeCall = RuntimeCall;
		type RuntimeEvent = RuntimeEvent;
		type RuntimeOrigin = RuntimeOrigin;
		type SS58Prefix = ();
		type SystemWeightInfo = ();
		type Version = ();
	}

	impl pallet_balances::Config for Runtime {
		type AccountStore = System;
		type Balance = u64;
		type DustRemoval = ();
		type ExistentialDeposit = ConstU64<1>;
		type FreezeIdentifier = ();
		type HoldIdentifier = ();
		type MaxFreezes = ();
		type MaxHolds = frame_support::traits::ConstU32<1>;
		type MaxLocks = ();
		type MaxReserves = ();
		type ReserveIdentifier = [u8; 8];
		type RuntimeEvent = RuntimeEvent;
		type WeightInfo = ();
	}

	parameter_types! {
		pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
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

	fn new_test_ext() -> TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime>::default()
			.assimilate_storage(&mut t)
			.unwrap();

		TestExternalities::new(t)
	}

	#[test]
	fn test_fees_and_tip_split() {
		new_test_ext().execute_with(|| {
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
