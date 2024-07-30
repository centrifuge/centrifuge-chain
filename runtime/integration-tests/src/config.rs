use std::fmt::Debug;

use cfg_primitives::{
	AccountId, Address, AuraId, Balance, CollectionId, Header, IBalance, InvestmentId, ItemId,
	LoanId, Nonce, OrderId, PoolId, Signature, TrancheId,
};
use cfg_traits::Millis;
use cfg_types::{
	fixed_point::{Quantity, Rate, Ratio},
	investments::InvestmentPortfolio,
	locations::RestrictedTransferLocation,
	oracles::OracleKey,
	permissions::{PermissionScope, Role},
	tokens::{AssetStringLimit, CurrencyId, CustomMetadata, FilterCurrency},
};
use fp_evm::PrecompileSet;
use fp_self_contained::{SelfContainedCall, UncheckedExtrinsic};
use frame_support::{
	dispatch::{DispatchInfo, GetDispatchInfo, PostDispatchInfo, RawOrigin},
	traits::{IsSubType, IsType, OriginTrait},
	Parameter,
};
use liquidity_pools_gateway_routers::DomainRouter;
use pallet_liquidity_pools::Message;
use pallet_transaction_payment::CurrencyAdapter;
use parity_scale_codec::Codec;
use runtime_common::{
	apis,
	evm::precompile::H160Addresses,
	fees::{DealWithFees, WeightToFee},
	instances,
	instances::CouncilCollective,
	oracle::Feeder,
	remarks::Remark,
	rewards::SingleCurrencyMovement,
};
use sp_core::{sr25519::Public, H256};
use sp_runtime::{
	scale_info::TypeInfo,
	traits::{
		AccountIdLookup, Block, Dispatchable, Get, MaybeSerializeDeserialize, Member, OpaqueKeys,
	},
	FixedI128,
};

/// Kind of runtime to check in runtime time
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
		Nonce = Nonce,
		AccountId = AccountId,
		RuntimeCall = Self::RuntimeCallExt,
		RuntimeEvent = Self::RuntimeEventExt,
		Lookup = AccountIdLookup<AccountId, ()>,
		RuntimeOrigin = Self::RuntimeOriginExt,
		Block = Self::BlockExt,
		Hash = H256,
	> + pallet_pool_system::Config<
		CurrencyId = CurrencyId,
		Balance = Balance,
		PoolId = PoolId,
		Rate = Rate,
		TrancheId = TrancheId,
		BalanceRatio = Quantity,
		MaxTranches = Self::MaxTranchesExt,
		TrancheCurrency = InvestmentId,
	> + pallet_balances::Config<Balance = Balance>
	+ pallet_pool_registry::Config<
		CurrencyId = CurrencyId,
		PoolId = PoolId,
		TrancheId = TrancheId,
		InterestRate = Rate,
		Balance = Balance,
		MaxTranches = Self::MaxTranchesExt,
		ModifyPool = pallet_pool_system::Pallet<Self>,
		ModifyWriteOffPolicy = pallet_loans::Pallet<Self>,
	> + pallet_permissions::Config<Role = Role, Scope = PermissionScope<PoolId, CurrencyId>>
	+ pallet_investments::Config<InvestmentId = InvestmentId, Amount = Balance, BalanceRatio = Ratio>
	+ pallet_loans::Config<
		Balance = Balance,
		PoolId = PoolId,
		LoanId = LoanId,
		CollectionId = CollectionId,
		ItemId = ItemId,
		Rate = Rate,
		Quantity = Quantity,
		PriceId = OracleKey,
		Moment = Millis,
	> + orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>
	+ orml_asset_registry::module::Config<
		AssetId = CurrencyId,
		CustomMetadata = CustomMetadata,
		Balance = Balance,
		StringLimit = AssetStringLimit,
	> + pallet_uniques::Config<CollectionId = CollectionId, ItemId = ItemId>
	+ pallet_timestamp::Config<Moment = Millis>
	+ pallet_aura::Config<Moment = Millis, AuthorityId = AuraId>
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
	+ staging_parachain_info::Config
	+ pallet_oracle_feed::Config<OracleKey = OracleKey, OracleValue = Ratio>
	+ pallet_oracle_collection::Config<
		OracleKey = OracleKey,
		OracleValue = Balance,
		FeederId = Feeder<Self::RuntimeOriginExt>,
		CollectionId = PoolId,
	> + orml_xtokens::Config<CurrencyId = CurrencyId, Balance = Balance>
	+ pallet_xcm::Config
	+ pallet_proxy::Config<RuntimeCall = Self::RuntimeCallExt>
	+ pallet_restricted_tokens::Config<Balance = Balance, CurrencyId = CurrencyId>
	+ pallet_restricted_xtokens::Config
	+ pallet_transfer_allowlist::Config<
		CurrencyId = FilterCurrency,
		Location = RestrictedTransferLocation,
	> + pallet_liquidity_pools::Config<
		CurrencyId = CurrencyId,
		Balance = Balance,
		PoolId = PoolId,
		TrancheId = TrancheId,
		BalanceRatio = Ratio,
	> + pallet_liquidity_pools_gateway::Config<Router = DomainRouter<Self>, Message = Message>
	+ pallet_xcm_transactor::Config<CurrencyId = CurrencyId>
	+ pallet_ethereum::Config
	+ pallet_ethereum_transaction::Config
	+ pallet_order_book::Config<
		BalanceIn = Balance,
		BalanceOut = Balance,
		CurrencyId = CurrencyId,
		OrderIdNonce = u64,
		Ratio = Ratio,
		FeederId = Feeder<Self::RuntimeOriginExt>,
	> + pallet_foreign_investments::Config<
		ForeignBalance = Balance,
		PoolBalance = Balance,
		TrancheBalance = Balance,
		InvestmentId = InvestmentId,
		CurrencyId = CurrencyId,
		OrderId = OrderId,
	> + pallet_preimage::Config
	+ pallet_collective::Config<CouncilCollective, Proposal = Self::RuntimeCallExt>
	+ pallet_democracy::Config<Currency = pallet_balances::Pallet<Self>>
	+ pallet_collator_selection::Config<Currency = pallet_balances::Pallet<Self>>
	+ pallet_collator_allowlist::Config<ValidatorId = AccountId>
	+ pallet_session::Config<Keys = Self::SessionKeysExt, ValidatorId = AccountId>
	+ pallet_evm_chain_id::Config
	+ pallet_evm::Config
	+ pallet_remarks::Config<RuntimeCall = Self::RuntimeCallExt, Remark = Remark>
	+ pallet_utility::Config<RuntimeCall = Self::RuntimeCallExt>
	+ pallet_rewards::Config<
		instances::BlockRewards,
		GroupId = u32,
		CurrencyId = CurrencyId,
		RewardMechanism = pallet_rewards::mechanism::base::Mechanism<
			Balance,
			IBalance,
			FixedI128,
			SingleCurrencyMovement,
		>,
	> + pallet_evm::Config<
		Runner = pallet_evm::runner::stack::Runner<Self>,
		Currency = pallet_balances::Pallet<Self>,
	> + pallet_block_rewards::Config<
		Rate = Rate,
		CurrencyId = CurrencyId,
		Balance = Balance,
		Rewards = pallet_rewards::Pallet<Self, instances::BlockRewards>,
	> + axelar_gateway_precompile::Config
	+ pallet_token_mux::Config<
		BalanceIn = Balance,
		BalanceOut = Balance,
		CurrencyId = CurrencyId,
		OrderId = OrderId,
	>
{
	/// Value to differentiate the runtime in tests.
	const KIND: RuntimeKind;

	/// Just the RuntimeCall type, but redefined with extra bounds.
	/// You can add `From` bounds in order to convert pallet calls to
	/// RuntimeCall in tests.
	type RuntimeCallExt: Parameter
		+ Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>
		+ GetDispatchInfo
		+ SelfContainedCall
		+ Sync
		+ Send
		+ Clone
		+ From<frame_system::Call<Self>>
		+ From<pallet_timestamp::Call<Self>>
		+ From<pallet_balances::Call<Self>>
		+ From<pallet_investments::Call<Self>>
		+ From<pallet_loans::Call<Self>>
		+ From<cumulus_pallet_parachain_system::Call<Self>>
		+ From<pallet_oracle_feed::Call<Self>>
		+ From<pallet_oracle_collection::Call<Self>>
		+ From<pallet_restricted_tokens::Call<Self>>
		+ From<pallet_restricted_xtokens::Call<Self>>
		+ From<pallet_preimage::Call<Self>>
		+ From<pallet_proxy::Call<Self>>
		+ From<pallet_collective::Call<Self, CouncilCollective>>
		+ From<pallet_democracy::Call<Self>>
		+ From<pallet_liquidity_pools_gateway::Call<Self>>
		+ From<pallet_remarks::Call<Self>>
		+ From<pallet_proxy::Call<Self>>
		+ From<pallet_utility::Call<Self>>
		+ From<pallet_collator_selection::Call<Self>>
		+ IsSubType<pallet_balances::Call<Self>>
		+ IsSubType<pallet_remarks::Call<Self>>
		+ IsSubType<pallet_proxy::Call<Self>>
		+ IsSubType<pallet_utility::Call<Self>>;

	/// Just the RuntimeEvent type, but redefined with extra bounds.
	/// You can add `TryInto` and `From` bounds in order to convert pallet
	/// events to RuntimeEvent in tests.
	type RuntimeEventExt: Parameter
		+ Member
		+ Debug
		+ IsType<<Self as frame_system::Config>::RuntimeEvent>
		+ TryInto<frame_system::Event<Self>>
		+ TryInto<pallet_balances::Event<Self>>
		+ TryInto<pallet_transaction_payment::Event<Self>>
		+ TryInto<pallet_loans::Event<Self>>
		+ TryInto<pallet_pool_system::Event<Self>>
		+ TryInto<pallet_liquidity_pools_gateway::Event<Self>>
		+ TryInto<pallet_proxy::Event<Self>>
		+ TryInto<pallet_ethereum::Event>
		+ TryInto<pallet_evm::Event<Self>>
		+ TryInto<pallet_collator_selection::Event<Self>>
		+ TryInto<pallet_rewards::Event<Self, instances::BlockRewards>>
		+ TryInto<pallet_block_rewards::Event<Self>>
		+ From<frame_system::Event<Self>>
		+ From<pallet_balances::Event<Self>>
		+ From<pallet_investments::Event<Self>>
		+ From<pallet_transaction_payment::Event<Self>>
		+ From<pallet_loans::Event<Self>>
		+ From<pallet_pool_system::Event<Self>>
		+ From<pallet_oracle_feed::Event<Self>>
		+ From<pallet_oracle_collection::Event<Self>>
		+ From<pallet_investments::Event<Self>>
		+ From<pallet_collator_selection::Event<Self>>
		+ From<orml_tokens::Event<Self>>
		+ From<pallet_liquidity_pools_gateway::Event<Self>>
		+ From<pallet_order_book::Event<Self>>
		+ From<pallet_preimage::Event<Self>>
		+ From<pallet_collective::Event<Self, CouncilCollective>>
		+ From<pallet_proxy::Event<Self>>
		+ From<pallet_democracy::Event<Self>>
		+ From<pallet_ethereum::Event>
		+ From<pallet_evm::Event<Self>>
		+ From<pallet_rewards::Event<Self, instances::BlockRewards>>
		+ From<pallet_block_rewards::Event<Self>>
		+ From<pallet_ethereum::Event>;

	type RuntimeOriginExt: Into<Result<RawOrigin<Self::AccountId>, <Self as frame_system::Config>::RuntimeOrigin>>
		+ From<RawOrigin<Self::AccountId>>
		+ Clone
		+ OriginTrait<Call = <Self as frame_system::Config>::RuntimeCall, AccountId = AccountId>
		+ From<pallet_ethereum::RawOrigin>
		+ Into<Result<pallet_ethereum::Origin, <Self as frame_system::Config>::RuntimeOrigin>>
		+ From<pallet_liquidity_pools_gateway::GatewayOrigin>;

	/// Block used by the runtime
	type BlockExt: Block<
		Hash = H256,
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
				frame_metadata_hash_extension::CheckMetadataHash<Self>,
				runtime_common::transfer_filter::PreBalanceTransferExtension<Self>,
			),
		>,
	>;

	/// You can extend this bounds to give extra API support
	type Api: sp_api::runtime_decl_for_core::CoreV4<Self::BlockExt>
		+ sp_block_builder::runtime_decl_for_block_builder::BlockBuilderV6<Self::BlockExt>
		+ apis::runtime_decl_for_loans_api::LoansApiV3<
			Self::BlockExt,
			PoolId,
			LoanId,
			pallet_loans::entities::loans::ActiveLoanInfo<Self>,
			Balance,
			pallet_loans::entities::input::PriceCollectionInput<Self>,
		> + apis::runtime_decl_for_pools_api::PoolsApiV1<
			Self::BlockExt,
			PoolId,
			TrancheId,
			Balance,
			CurrencyId,
			Quantity,
			Self::MaxTranchesExt,
		> + apis::runtime_decl_for_investments_api::InvestmentsApiV1<
			Self::BlockExt,
			AccountId,
			InvestmentId,
			InvestmentPortfolio<Balance, CurrencyId>,
		> + apis::runtime_decl_for_account_conversion_api::AccountConversionApiV1<
			Self::BlockExt,
			AccountId,
		> + apis::runtime_decl_for_rewards_api::RewardsApiV1<
			Self::Block,
			AccountId,
			Balance,
			CurrencyId,
		>;

	type MaxTranchesExt: Codec + Get<u32> + Member + PartialOrd + TypeInfo;

	type SessionKeysExt: OpaqueKeys + Member + Parameter + MaybeSerializeDeserialize;

	type PrecompilesTypeExt: PrecompileSet + H160Addresses;

	fn initialize_session_keys(public_id: Public) -> Self::SessionKeysExt;
}
