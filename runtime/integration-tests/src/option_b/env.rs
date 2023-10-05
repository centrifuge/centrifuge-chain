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
use fp_self_contained::UncheckedExtrinsic;
use frame_support::{
	dispatch::{DispatchInfo, PostDispatchInfo, UnfilteredDispatchable},
	inherent::{InherentData, ProvideInherent},
	Parameter,
};
use frame_system::RawOrigin;
use pallet_transaction_payment::CurrencyAdapter;
use runtime_common::fees::{DealWithFees, WeightToFee};
use sp_io::TestExternalities;
use sp_runtime::{
	traits::{AccountIdLookup, Block, Dispatchable, Extrinsic},
	ApplyExtrinsicResult,
};
use sp_timestamp::Timestamp;

use crate::utils::accounts::Keyring;

pub enum RuntimeKind {
	Development,
	Altair,
	Centrifuge,
}

pub trait Config:
	Send
	+ Sync
	+ frame_system::Config<
		Index = Index,
		AccountId = AccountId,
		RuntimeCall = Self::RuntimeCallExt,
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
	+ pallet_treasury::Config<Currency = pallet_restricted_tokens::Pallet<Self>>
	+ pallet_transaction_payment::Config<
		WeightToFee = WeightToFee,
		OnChargeTransaction = CurrencyAdapter<pallet_balances::Pallet<Self>, DealWithFees<Self>>,
	> + pallet_restricted_tokens::Config<
		Balance = Balance,
		NativeFungible = pallet_balances::Pallet<Self>,
	> + cumulus_pallet_parachain_system::Config
{
	// Just the RuntimeCall type, but redefined with extra bounds.
	// You can add `From` bounds in order to convert pallet calls to RuntimeCall in
	// tests.
	type RuntimeCallExt: Parameter
		+ Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>
		+ From<frame_system::Call<Self>>
		+ From<pallet_timestamp::Call<Self>>
		+ From<pallet_balances::Call<Self>>
		+ From<cumulus_pallet_parachain_system::Call<Self>>;

	// Actual extrinsic type used by the runtime
	type Extrinsic: Extrinsic<
			Call = Self::RuntimeCallExt,
			SignaturePayload = (
				Address,
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
			),
		> + Debug;

	// Block used by the runtime
	type Block: Block<Header = Header, Extrinsic = Self::Extrinsic>;

	// Value to differentiate the runtime in tests.
	const KIND: RuntimeKind;

	fn execute_block(header: Self::Block);
	fn initialize_block(header: &<Self::Block as Block>::Header);
	fn apply_extrinsic(extrinsic: Self::Extrinsic) -> ApplyExtrinsicResult;
	fn finalize_block() -> <Self::Block as Block>::Header;
}

pub trait Env<T: Config> {
	fn submit(&mut self, who: Keyring, call: impl Into<T::RuntimeCall>) -> ApplyExtrinsicResult;
	fn pass(&mut self, blocks: u32);
	fn state(&mut self, f: impl FnOnce());
}
