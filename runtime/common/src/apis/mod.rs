use pallet_anchors::AnchorData;
use pallet_pools::{EpochSolution, TrancheIndex, TrancheLoc, TrancheSolution};

use sp_api::decl_runtime_apis;

decl_runtime_apis! {
	/// Runtime for pallet-anchors.
	///
	/// Note: That the runtime api is pallet specific, while the rpc method
	///       are more focused on domain-specifc logic	pub trait AnchorApi<Hash, BlockNumber>
	where
		Hash: Codec,
		BlockNumber: Codec
	{
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
	}
}

decl_runtime_apis! {
	/// Runtime for pallet-loans.
	///
	/// Note: That the runtime api is pallet specific, while the rpc method
	///       are more focused on domain-specifc logic
	pub trait LoansApi<PoolId, LoanId, Balance>
	where
		PoolId: Codec,
		LoanId: Codec,
		Balance: Codec,
	{
		fn nav(id: PoolId) -> Option<Balance>;

		fn max_borrow_amount(id: PoolId, loan_id: LoanId) -> Option<Balance>;
	}
}

decl_runtime_apis! {
	/// Runtime for pallet-pools.
	///
	/// Note: That the runtime api is pallet specific, while the rpc method
	///       are more focused on domain-specifc logic
	pub trait PoolsApi<PoolId, TrancheId, Balance, Currency, BalanceRatio>
	where
		PoolId: Codec,
		TrancheId: Codec,
		Balance: Codec,
		Currency: Codec,
		BalanceRatio: Codec,
	{
		fn pool_value(pool_id: PoolId) -> Option<Balance>;

		fn pool_currency(poold_id: PoolId) -> Option<Currency>;

		fn inspect_epoch_solution(pool_id: PoolId, solution: Vec<TrancheSolution>) -> Option<EpochSolution<Balance>>;

		fn tranche_token_price(pool_id: PoolId, tranche: TrancheLoc<TrancheId>) -> Option<BalanceRatio>;

		fn tranche_token_prices(pool_id: PoolId) -> Option<Vec<BalanceRatio>>;

		fn tranche_ids(pool_id: PoolId) -> Option<Vec<TrancheId>>;

		fn tranche_id(pool_id: PoolId, tranche_index: TrancheIndex) -> Option<TrancheId>;

		fn tranche_currency(pool_id, tranche_loc: TrancheLoc<TrancheId>) -> Option<Currency>;
	}
}
