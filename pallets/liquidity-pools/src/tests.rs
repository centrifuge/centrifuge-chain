use cfg_traits::liquidity_pools::InboundQueue;
use cfg_types::{
	domain_address::DomainAddress,
	tokens::{
		default_metadata, AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata,
	},
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{fungibles::Mutate as _, PalletInfo as _},
};
use sp_runtime::{DispatchError, TokenError};
use staging_xcm::{
	v4::{Junction::*, Location, NetworkId},
	VersionedLocation,
};

use crate::{mock::*, Error, GeneralCurrencyIndexOf, Message};

const CHAIN_ID: u64 = 1;
const ALICE: AccountId = AccountId::new([1; 32]);
const CONTRACT_ACCOUNT: [u8; 20] = [1; 20];
const EVM_ADDRESS: DomainAddress = DomainAddress::EVM(CHAIN_ID, CONTRACT_ACCOUNT);
const AMOUNT: Balance = 100;
const CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

fn transferable_metadata() -> AssetMetadata {
	AssetMetadata {
		additional: CustomMetadata {
			transferability: CrossChainTransferability::LiquidityPools,
			..Default::default()
		},
		..default_metadata()
	}
}

fn wrapped_transferable_metadata() -> AssetMetadata {
	let pallet_index = PalletInfo::index::<LiquidityPools>();
	AssetMetadata {
		location: Some(VersionedLocation::V4(Location::new(
			0,
			[
				PalletInstance(pallet_index.unwrap() as u8),
				GlobalConsensus(NetworkId::Ethereum { chain_id: CHAIN_ID }),
				AccountKey20 {
					network: None,
					key: CONTRACT_ACCOUNT,
				},
			],
		))),
		..transferable_metadata()
	}
}

fn currency_index(currency_id: CurrencyId) -> u128 {
	GeneralCurrencyIndexOf::<Runtime>::try_from(currency_id)
		.unwrap()
		.index
}

mod erroring_out_when {
	use super::*;

	#[test]
	fn receiving_output_message() {
		System::externalities().execute_with(|| {
			let msg = Message::AddPool { pool_id: 123 };

			assert_noop!(
				LiquidityPools::submit(EVM_ADDRESS, msg),
				Error::<Runtime>::InvalidIncomingMessage,
			);
		})
	}

	mod transfer {
		use super::*;

		#[test]
		fn zero_balance() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_ADDRESS,
						0
					),
					Error::<Runtime>::InvalidTransferAmount,
				);
			})
		}

		#[test]
		fn tranche_currency() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CurrencyId::Tranche(42, [0; 16]),
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::InvalidTransferCurrency,
				);
			})
		}

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| None);

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}

		#[test]
		fn with_unsupported_token() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(default_metadata()));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CurrencyId::Native,
						EVM_ADDRESS,
						AMOUNT
					),
					TokenError::Unsupported,
				);
			})
		}

		#[test]
		fn no_transferible_asset() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(default_metadata()));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::AssetNotLiquidityPoolsTransferable,
				);
			})
		}

		#[test]
		fn without_correct_location() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(transferable_metadata()));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::AssetNotLiquidityPoolsWrappedToken
				);
			})
		}

		#[test]
		fn without_correct_domain() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(wrapped_transferable_metadata()));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						DomainAddress::Centrifuge([2; 32]),
						AMOUNT
					),
					Error::<Runtime>::InvalidDomain
				);
			})
		}

		#[test]
		fn without_satisfy_lp_filter() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(wrapped_transferable_metadata()));
				TransferFilter::mock_check(|_| Err(DispatchError::Other("Err")));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					DispatchError::Other("Err"),
				);
			})
		}

		#[test]
		fn without_sufficient_balance() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(wrapped_transferable_metadata()));
				TransferFilter::mock_check(|_| Ok(()));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::BalanceTooLow
				);
			})
		}
	}
}

#[test]
fn correct_transfer() {
	System::externalities().execute_with(|| {
		AssetRegistry::mock_metadata(|_| Some(wrapped_transferable_metadata()));
		TransferFilter::mock_check(|_| Ok(()));
		Tokens::mint_into(CURRENCY_ID, &ALICE, AMOUNT).unwrap();

		Gateway::mock_submit(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, EVM_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::Transfer {
					currency: currency_index(CURRENCY_ID),
					sender: ALICE.into(),
					receiver: EVM_ADDRESS.address(),
					amount: AMOUNT
				}
			);
			Ok(())
		});

		assert_ok!(LiquidityPools::transfer(
			RuntimeOrigin::signed(ALICE),
			CurrencyId::ForeignAsset(1),
			EVM_ADDRESS,
			AMOUNT
		));
	})
}
