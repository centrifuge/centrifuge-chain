use std::fmt::Debug;

use cfg_primitives::{
	AccountId, Address, AuraId, Balance, BlockNumber, CollectionId, CouncilCollective, Header,
	IBalance, Index, ItemId, LoanId, OrderId, PoolId, Signature, TrancheId,
};
use cfg_traits::Millis;
use cfg_types::{
	domain_address::Domain,
	fixed_point::{Quantity, Rate, Ratio},
	investments::InvestmentPortfolio,
	locations::Location,
	oracles::OracleKey,
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, FilterCurrency, TrancheCurrency},
};
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
	fees::{DealWithFees, WeightToFee},
	oracle::Feeder,
	remarks::Remark,
	rewards::SingleCurrencyMovement,
};
use sp_core::H256;
use sp_runtime::{
	scale_info::TypeInfo,
	traits::{AccountIdLookup, Block, Dispatchable, Get, Member},
	FixedI128,
};

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
		RuntimeOrigin = Self::RuntimeOriginExt,
		Hash = H256,
	> + pallet_pool_system::Config<
		CurrencyId = CurrencyId,
		Balance = Balance,
		PoolId = PoolId,
		Rate = Rate,
		TrancheId = TrancheId,
		BalanceRatio = Quantity,
		MaxTranches = Self::MaxTranchesExt,
	> + pallet_balances::Config<Balance = Balance>
	+ pallet_pool_registry::Config<
		CurrencyId = CurrencyId,
		PoolId = PoolId,
		InterestRate = Rate,
		Balance = Balance,
		MaxTranches = Self::MaxTranchesExt,
		ModifyPool = pallet_pool_system::Pallet<Self>,
		ModifyWriteOffPolicy = pallet_loans::Pallet<Self>,
	> + pallet_permissions::Config<Role = Role, Scope = PermissionScope<PoolId, CurrencyId>>
	+ pallet_investments::Config<
		InvestmentId = TrancheCurrency,
		Amount = Balance,
		BalanceRatio = Ratio,
	> + pallet_loans::Config<
		Balance = Balance,
		PoolId = PoolId,
		LoanId = LoanId,
		CollectionId = CollectionId,
		ItemId = ItemId,
		Rate = Rate,
		Quantity = Quantity,
		PriceId = OracleKey,
	> + orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>
	+ orml_asset_registry::Config<
		AssetId = CurrencyId,
		CustomMetadata = CustomMetadata,
		Balance = Balance,
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
	+ parachain_info::Config
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
	+ pallet_transfer_allowlist::Config<CurrencyId = FilterCurrency, Location = Location>
	+ pallet_liquidity_pools::Config<
		CurrencyId = CurrencyId,
		Balance = Balance,
		PoolId = PoolId,
		TrancheId = TrancheId,
		TrancheCurrency = TrancheCurrency,
		BalanceRatio = Ratio,
	> + pallet_liquidity_pools_gateway::Config<
		Router = DomainRouter<Self>,
		Message = Message<Domain, PoolId, TrancheId, Balance, Quantity>,
	> + pallet_xcm_transactor::Config<CurrencyId = CurrencyId>
	+ pallet_ethereum::Config
	+ pallet_ethereum_transaction::Config
	+ pallet_order_book::Config<
		BalanceIn = Balance,
		BalanceOut = Balance,
		CurrencyId = CurrencyId,
		OrderIdNonce = u64,
		Ratio = Ratio,
		FeederId = Feeder<Self::RuntimeOriginExt>,
	> + pallet_swaps::Config<OrderId = OrderId, SwapId = pallet_foreign_investments::SwapId<Self>>
	+ pallet_foreign_investments::Config<
		ForeignBalance = Balance,
		PoolBalance = Balance,
		TrancheBalance = Balance,
		InvestmentId = TrancheCurrency,
		CurrencyId = CurrencyId,
	> + pallet_preimage::Config
	+ pallet_collective::Config<CouncilCollective, Proposal = Self::RuntimeCallExt>
	+ pallet_democracy::Config<Currency = pallet_balances::Pallet<Self>>
	+ pallet_evm_chain_id::Config
	+ pallet_remarks::Config<RuntimeCall = Self::RuntimeCallExt, Remark = Remark>
	+ pallet_utility::Config<RuntimeCall = Self::RuntimeCallExt>
	+ pallet_rewards::Config<
		pallet_rewards::Instance1,
		GroupId = u32,
		CurrencyId = CurrencyId,
		RewardMechanism = pallet_rewards::mechanism::base::Mechanism<
			Balance,
			IBalance,
			FixedI128,
			SingleCurrencyMovement,
		>,
	>
	+ pallet_evm::Config<
		Runner = pallet_evm::runner::stack::Runner<Self>,
		Currency = pallet_balances::Pallet<Self>,
	> + axelar_gateway_precompile::Config
{
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
		+ From<frame_system::Event<Self>>
		+ From<pallet_balances::Event<Self>>
		+ From<pallet_investments::Event<Self>>
		+ From<pallet_transaction_payment::Event<Self>>
		+ From<pallet_loans::Event<Self>>
		+ From<pallet_pool_system::Event<Self>>
		+ From<pallet_oracle_feed::Event<Self>>
		+ From<pallet_oracle_collection::Event<Self>>
		+ From<pallet_investments::Event<Self>>
		+ From<orml_tokens::Event<Self>>
		+ From<pallet_liquidity_pools_gateway::Event<Self>>
		+ From<pallet_order_book::Event<Self>>
		+ From<pallet_preimage::Event<Self>>
		+ From<pallet_collective::Event<Self, CouncilCollective>>
		+ From<pallet_proxy::Event<Self>>
		+ From<pallet_democracy::Event<Self>>;

	type RuntimeOriginExt: Into<Result<RawOrigin<Self::AccountId>, <Self as frame_system::Config>::RuntimeOrigin>>
		+ From<RawOrigin<Self::AccountId>>
		+ Clone
		+ OriginTrait<Call = <Self as frame_system::Config>::RuntimeCall, AccountId = AccountId>
		+ From<pallet_ethereum::RawOrigin>
		+ Into<Result<pallet_ethereum::Origin, <Self as frame_system::Config>::RuntimeOrigin>>
		+ From<pallet_liquidity_pools_gateway::GatewayOrigin>;

	/// Block used by the runtime
	type Block: Block<
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
				runtime_common::transfer_filter::PreBalanceTransferExtension<Self>,
			),
		>,
	>;

	/// You can extend this bounds to give extra API support
	type Api: sp_api::runtime_decl_for_core::CoreV4<Self::Block>
		+ sp_block_builder::runtime_decl_for_block_builder::BlockBuilderV6<Self::Block>
		+ apis::runtime_decl_for_loans_api::LoansApiV2<
			Self::Block,
			PoolId,
			LoanId,
			pallet_loans::entities::loans::ActiveLoanInfo<Self>,
			Balance,
			pallet_loans::entities::input::PriceCollectionInput<Self>,
		> + apis::runtime_decl_for_pools_api::PoolsApiV1<
			Self::Block,
			PoolId,
			TrancheId,
			Balance,
			CurrencyId,
			Quantity,
			Self::MaxTranchesExt,
		> + apis::runtime_decl_for_investments_api::InvestmentsApiV1<
			Self::Block,
			AccountId,
			TrancheCurrency,
			InvestmentPortfolio<Balance, CurrencyId>,
		> + apis::runtime_decl_for_account_conversion_api::AccountConversionApiV1<
			Self::Block,
			AccountId,
		> + apis::runtime_decl_for_rewards_api::RewardsApiV1<
			Self::Block,
			AccountId,
			Balance,
			CurrencyId,
		>;

	type MaxTranchesExt: Codec + Get<u32> + Member + PartialOrd + TypeInfo;

	/// Value to differentiate the runtime in tests.
	const KIND: RuntimeKind;
}
