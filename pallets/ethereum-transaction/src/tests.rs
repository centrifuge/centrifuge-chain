use cfg_traits::ethereum::EthereumTransactor;
use frame_support::{assert_ok, traits::fungible::Mutate};
use pallet_evm::{AddressMapping, Error::BalanceLow};
use sp_core::{crypto::AccountId32, H160, U256};
use sp_runtime::DispatchError;

use super::mock::*;
use crate::pallet::Nonce;

mod utils {
	use super::*;

	pub fn get_test_account_id() -> AccountId32 {
		[0u8; 32].into()
	}

	pub fn get_test_data() -> [u8; 10] {
		[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
	}
}

use utils::*;

mod call {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let sender: AccountId32 = get_test_account_id();
			let sender_h160: H160 =
				H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender)[0..20]);
			let derived_sender = IdentityAddressMapping::into_account_id(sender_h160);

			Balances::mint_into(&derived_sender.into(), 1_000_000_000_000_000).unwrap();

			let to = H160::from_low_u64_be(2);
			let data = get_test_data();
			let value = U256::from(10);
			let gas_price = U256::from(10);

			let transaction_call_cost =
				<Runtime as pallet_evm::Config>::config().gas_transaction_call;

			// Ensure that the gas limit is enough to cover for executing a call.
			let gas_limit = U256::from(transaction_call_cost + 10_000);

			assert_eq!(Nonce::<Runtime>::get(), U256::from(0));

			assert_ok!(<EthereumTransaction as EthereumTransactor>::call(
				sender_h160,
				to,
				data.as_slice(),
				value,
				gas_price,
				gas_limit
			));

			assert_eq!(Nonce::<Runtime>::get(), U256::from(1));
		});
	}

	#[test]
	fn insufficient_balance() {
		new_test_ext().execute_with(|| {
			let sender: AccountId32 = get_test_account_id();
			let sender_h160: H160 =
				H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender)[0..20]);

			let to = H160::from_low_u64_be(2);
			let data = get_test_data();
			let value = U256::from(0);
			let gas_price = U256::from(10);

			let transaction_call_cost =
				<Runtime as pallet_evm::Config>::config().gas_transaction_call;

			let gas_limit = U256::from(transaction_call_cost + 10_000);

			assert_eq!(Nonce::<Runtime>::get(), U256::from(0));

			let res = <EthereumTransaction as EthereumTransactor>::call(
				sender_h160,
				to,
				data.as_slice(),
				value,
				gas_price,
				gas_limit,
			);
			assert_eq!(res.err().unwrap().error, BalanceLow::<Runtime>.into());

			assert_eq!(Nonce::<Runtime>::get(), U256::from(1));
		});
	}

	#[test]
	fn out_of_gas() {
		new_test_ext().execute_with(|| {
			let sender: AccountId32 = get_test_account_id();
			let sender_h160: H160 =
				H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender)[0..20]);
			let derived_sender = IdentityAddressMapping::into_account_id(sender_h160);

			Balances::mint_into(&derived_sender.into(), 1_000_000_000_000_000).unwrap();

			let to = H160::from_low_u64_be(2);
			let data = get_test_data();
			let value = U256::from(0);
			let gas_price = U256::from(10);

			let transaction_call_cost =
				<Runtime as pallet_evm::Config>::config().gas_transaction_call;

			let gas_limit = U256::from(transaction_call_cost - 10_000);

			assert_eq!(Nonce::<Runtime>::get(), U256::from(0));

			let res = <EthereumTransaction as EthereumTransactor>::call(
				sender_h160,
				to,
				data.as_slice(),
				value,
				gas_price,
				gas_limit,
			);
			assert_eq!(res.err().unwrap().error, DispatchError::Other("out of gas"));

			assert_eq!(Nonce::<Runtime>::get(), U256::from(1));
		});
	}
}
