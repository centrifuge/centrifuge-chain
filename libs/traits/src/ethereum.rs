use frame_support::dispatch::DispatchResultWithPostInfo;
use sp_runtime::app_crypto::sp_core::{H160, U256};

/// Something capable of managing transactions in an EVM/Ethereum context
pub trait EthereumTransactor {
	/// Transacts the specified call in the EVM context,
	/// exposing the call and any events to the EVM block.
	fn call(
		from: H160,
		to: H160,
		data: &[u8],
		value: U256,
		gas_price: U256,
		gas_limit: U256,
	) -> DispatchResultWithPostInfo;
}
