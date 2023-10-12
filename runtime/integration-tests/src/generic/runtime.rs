use std::fmt::Debug;

use cfg_primitives::{
	AccountId, Address, AuraId, Balance, BlockNumber, CollectionId, Header, Index, ItemId, LoanId,
	Moment, PoolId, Signature, TrancheId,
};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use fp_self_contained::{SelfContainedCall, UncheckedExtrinsic};
use frame_support::{
	dispatch::{DispatchInfo, GetDispatchInfo, PostDispatchInfo},
	traits::IsType,
	Parameter,
};
use pallet_transaction_payment::CurrencyAdapter;
use runtime_common::{
	apis,
	fees::{DealWithFees, WeightToFee},
};
use sp_runtime::traits::{AccountIdLookup, Block, Dispatchable, Member};

/// Kind of runtime to check in runtime time
pub enum RuntimeKind {
	Development,
	Altair,
	Centrifuge,
}

/// Runtime configuration
pub trait Runtime:
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
	+ pallet_treasury::Config<Currency = pallet_restricted_tokens::Pallet<Self>>
	+ pallet_transaction_payment::Config<
        AccountId = AccountId,
		WeightToFee = WeightToFee,
		OnChargeTransaction = CurrencyAdapter<pallet_balances::Pallet<Self>, DealWithFees<Self>>,
	> + pallet_restricted_tokens::Config<
		Balance = Balance,
		NativeFungible = pallet_balances::Pallet<Self>,
	> + cumulus_pallet_parachain_system::Config

    // APIS:
    + sp_api::runtime_decl_for_Core::CoreV4<Self::Block>
    + sp_block_builder::runtime_decl_for_BlockBuilder::BlockBuilderV6<Self::Block>
	+ apis::runtime_decl_for_LoansApi::LoansApiV1<
		Self::Block,
		PoolId,
		LoanId,
		pallet_loans::entities::loans::ActiveLoanInfo<Self>,
	>
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
		+ TryInto<pallet_transaction_payment::Event<Self>>
		+ From<frame_system::Event<Self>>
		+ From<pallet_balances::Event<Self>>
		+ From<pallet_transaction_payment::Event<Self>>;

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
}
