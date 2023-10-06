use std::fmt::Debug;

use cfg_primitives::{
	AccountId, Address, AuraId, Balance, BlockNumber, CollectionId, Header, Index, ItemId, Moment,
	PoolId, Signature, TrancheId,
};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use codec::Codec;
use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use fp_self_contained::{SelfContainedCall, UncheckedExtrinsic};
use frame_support::{
	dispatch::{
		DispatchClass, DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo,
		UnfilteredDispatchable,
	},
	inherent::{InherentData, ProvideInherent},
	traits::{Get, IsType},
	weights::WeightToFee as _,
	Parameter,
};
use frame_system::{ChainContext, RawOrigin};
use pallet_transaction_payment::CurrencyAdapter;
use runtime_common::fees::{DealWithFees, WeightToFee};
use sp_io::TestExternalities;
use sp_runtime::{
	traits::{AccountIdLookup, Block, Checkable, Dispatchable, Extrinsic, Lookup, Member},
	ApplyExtrinsicResult, DispatchResult,
};
use sp_timestamp::Timestamp;

use crate::{generic::utils::genesis::Genesis, utils::accounts::Keyring};

/// Kind of runtime to check in runtime time
pub enum RuntimeKind {
	Development,
	Altair,
	Centrifuge,
}

/// Runtime configuration
pub trait Config:
	Send
	+ Sync
	+ frame_system::Config<
		Index = Index,
		AccountId = AccountId,
		RuntimeCall = Self::RuntimeCallExt,
		RuntimeEvent = Self::RuntimeEventExt,
		BlockNumber = BlockNumber,
		Lookup = AccountIdLookup<AccountId, ()>,
	> + pallet_pool_system::Config<
		CurrencyId = CurrencyId,
		Balance = Balance,
		PoolId = PoolId,
		TrancheId = TrancheId,
	> + pallet_balances::Config<Balance = Balance>
	+ pallet_investments::Config<InvestmentId = TrancheCurrency, Amount = Balance>
	+ pallet_pool_registry::Config<
		CurrencyId = CurrencyId,
		PoolId = PoolId,
		Balance = Balance,
		ModifyPool = pallet_pool_system::Pallet<Self>,
		ModifyWriteOffPolicy = pallet_loans::Pallet<Self>,
	> + pallet_permissions::Config<Role = Role, Scope = PermissionScope<PoolId, CurrencyId>>
	+ pallet_loans::Config<
		Balance = Balance,
		PoolId = PoolId,
		CollectionId = CollectionId,
		ItemId = ItemId,
	> + orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>
	+ orml_asset_registry::Config<
		AssetId = CurrencyId,
		CustomMetadata = CustomMetadata,
		Balance = Balance,
	> + pallet_uniques::Config<CollectionId = CollectionId, ItemId = ItemId>
	+ pallet_timestamp::Config<Moment = Moment>
	+ pallet_aura::Config<Moment = Moment, AuthorityId = AuraId>
	+ pallet_authorship::Config
	+ pallet_treasury::Config<Currency = pallet_balances::Pallet<Self>>
	+ pallet_transaction_payment::Config<
		WeightToFee = WeightToFee,
		OnChargeTransaction = CurrencyAdapter<pallet_balances::Pallet<Self>, DealWithFees<Self>>,
	> + pallet_restricted_tokens::Config<
		Balance = Balance,
		NativeFungible = pallet_balances::Pallet<Self>,
	> + cumulus_pallet_parachain_system::Config
{
	/// Just the RuntimeCall type, but redefined with extra bounds.
	/// You can add `From` bounds in order to convert pallet calls to
	/// RuntimeCall in tests.
	type RuntimeCallExt: Parameter
		+ Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>
		+ GetDispatchInfo
		+ SelfContainedCall
		+ From<frame_system::Call<Self>>
		+ From<pallet_timestamp::Call<Self>>
		+ From<pallet_balances::Call<Self>>
		+ From<cumulus_pallet_parachain_system::Call<Self>>;

	/// Just the RuntimeEvent type, but redefined with extra bounds.
	/// You can add `TryInto` and `From` bounds in order to convert pallet
	/// events to RuntimeEvent in tests.
	type RuntimeEventExt: Parameter
		+ Member
		+ From<frame_system::Event<Self>>
		+ Debug
		+ IsType<<Self as frame_system::Config>::RuntimeEvent>
		+ TryInto<frame_system::Event<Self>>
		+ TryInto<pallet_balances::Event<Self>>
		+ From<frame_system::Event<Self>>
		+ From<pallet_balances::Event<Self>>;

	/// Block used by the runtime
	type Block: Block<
		Header = Header,
		Extrinsic = UncheckedExtrinsic<
			Address,
			Self::RuntimeCallExt,
			Signature,
			(
				frame_system::CheckNonZeroSender<Self>,
				frame_system::CheckSpecVersion<Self>,
				frame_system::CheckTxVersion<Self>,
				frame_system::CheckGenesis<Self>,
				frame_system::CheckEra<Self>,
				frame_system::CheckNonce<Self>,
				frame_system::CheckWeight<Self>,
				pallet_transaction_payment::ChargeTransactionPayment<Self>,
			),
		>,
	>;

	/// Value to differentiate the runtime in tests.
	const KIND: RuntimeKind;

	fn initialize_block(header: &<Self::Block as Block>::Header);
	fn apply_extrinsic(extrinsic: <Self::Block as Block>::Extrinsic) -> ApplyExtrinsicResult;
	fn finalize_block() -> <Self::Block as Block>::Header;
}

/// Used by Env::pass() to determine how many blocks should be passed
#[derive(Clone)]
pub enum Blocks<T: Config> {
	/// Pass X blocks
	ByNumber(BlockNumber),

	/// Pass a number of blocks proportional to these seconds
	BySeconds(Moment),

	/// Pass a number of block until find an event or reach the limit
	UntilEvent {
		event: T::RuntimeEventExt,
		limit: BlockNumber,
	},
}

/// Define an environment behavior
pub trait Env<T: Config> {
	/// Loan the environment from a genesis
	fn from_genesis(genesis: Genesis) -> Self;

	/// Submit an extrinsic mutating the state
	fn submit(&mut self, who: Keyring, call: impl Into<T::RuntimeCall>) -> DispatchResult;

	/// Pass any number of blocks
	fn pass(&mut self, blocks: Blocks<T>);

	/// Allows to mutate the storage state through the closure
	fn state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R;

	/// Allows to read the storage state through the closure
	/// If storage is modified, it would not be applied.
	fn state<R>(&self, f: impl FnOnce() -> R) -> R;

	/// Check for an event introduced in the current block
	fn has_event(&self, event: impl Into<T::RuntimeEventExt>) -> bool {
		self.state(|| {
			let event = event.into();
			frame_system::Pallet::<T>::events()
				.into_iter()
				.find(|record| record.event == event)
				.is_some()
		})
	}

	/// Retrieve the fees used in the last submit call
	fn last_xt_fees(&self) -> Balance {
		self.state(|| {
			let runtime_event = frame_system::Pallet::<T>::events()
				.last()
				.unwrap()
				.clone()
				.event;

			let dispatch_info = match runtime_event.try_into() {
				Ok(frame_system::Event::<T>::ExtrinsicSuccess { dispatch_info }) => dispatch_info,
				_ => panic!("expected to be called after a successful extrinsic"),
			};

			match dispatch_info.pays_fee {
				Pays::Yes => WeightToFee::weight_to_fee(&dispatch_info.weight),
				Pays::No => 0,
			}
		})
	}
}
