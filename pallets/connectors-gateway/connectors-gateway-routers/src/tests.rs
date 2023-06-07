use sp_core::crypto::AccountId32;

use super::mock::*;

mod utils {
	use super::*;

	pub fn get_random_test_account_id() -> AccountId32 {
		rand::random::<[u8; 32]>().into()
	}
}

use utils::*;

mod ethereum_xcm {
	use cfg_mocks::MessageMock;
	use cfg_traits::connectors::Router;
	use frame_support::assert_ok;
	use pallet_xcm_transactor::RemoteTransactInfoWithMaxWeight;
	use sp_core::{bounded_vec, H160};
	use sp_runtime::traits::Convert;

	use super::*;
	use crate::{ethereum_xcm::EthereumXCMRouter, DomainRouter, XcmDomain};

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			// Set the correct currency_id and destination to ensure that
			// `pallet_xcm_transactor::Pallet::transfer_allowed` does not fail.

			let currency_id = CurrencyId::OtherReserve(1);
			let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

			let xcm_domain = XcmDomain {
				location: Box::new(dest.clone().versioned()),
				ethereum_xcm_transact_call_index: bounded_vec![0],
				contract_address: H160::from_slice(rand::random::<[u8; 20]>().as_slice()),
				fee_currency: currency_id,
				max_gas_limit: 10,
			};

			let domain_router =
				DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
					xcm_domain,
					_marker: Default::default(),
				});

			// Required in `pallet_xcm_transactor::Pallet::take_weight_from_transact_info`.

			pallet_xcm_transactor::TransactInfoWithWeightLimit::<Runtime>::insert(
				dest.clone(),
				RemoteTransactInfoWithMaxWeight {
					transact_extra_weight: 1,
					max_weight: 100_000_000_000,
					transact_extra_weight_signed: None,
				},
			);

			// Required in
			// `pallet_xcm_transactor::Pallet::take_fee_per_second_from_storage`.

			let fee_per_second = 1u128;

			pallet_xcm_transactor::DestinationAssetFeePerSecond::<Runtime>::insert(
				dest,
				fee_per_second,
			);

			let sender = get_random_test_account_id();
			let msg = MessageMock::Second;

			assert_ok!(domain_router.send(sender, msg));
		});
	}
}
