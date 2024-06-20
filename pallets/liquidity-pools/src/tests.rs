use cfg_primitives::{PoolId, TrancheId};
use cfg_traits::{liquidity_pools::InboundQueue, Millis};
use cfg_types::{
	domain_address::DomainAddress,
	permissions::{PermissionScope, PoolRole, Role},
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
const ALICE: AccountId = AccountId::new([0; 32]);
const CONTRACT_ACCOUNT: [u8; 20] = [1; 20];
const CONTRACT_ACCOUNT_ID: AccountId = AccountId::new([1; 32]);
const EVM_ADDRESS: DomainAddress = DomainAddress::EVM(CHAIN_ID, CONTRACT_ACCOUNT);
const AMOUNT: Balance = 100;
const CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);
const POOL_ID: PoolId = 1;
const TRANCHE_ID: TrancheId = [1; 16];
const NOW: Millis = 0;

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

mod transfer {
	use super::*;

	#[test]
	fn success() {
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

	mod erroring_out_when {
		use super::*;

		#[test]
		fn with_zero_balance() {
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
		fn with_tranche_currency() {
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
		fn with_no_transferible_asset() {
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

mod transfer_tranche_tokens {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
			Time::mock_now(|| NOW);
			Permissions::mock_has(move |scope, who, role| {
				assert_eq!(who, CONTRACT_ACCOUNT_ID);
				assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
				assert!(matches!(
					role,
					Role::PoolRole(PoolRole::TrancheInvestor(TRANCHE_ID, NOW))
				));
				true
			});
			Pools::mock_pool_exists(|_| true);
			Pools::mock_tranche_exists(|_, _| true);
			TransferFilter::mock_check(|_| Ok(()));
			Tokens::mint_into(CurrencyId::Tranche(POOL_ID, TRANCHE_ID), &ALICE, AMOUNT).unwrap();

			Gateway::mock_submit(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::TransferTrancheTokens {
						pool_id: POOL_ID,
						tranche_id: TRANCHE_ID,
						sender: ALICE.into(),
						domain: EVM_ADDRESS.domain(),
						receiver: EVM_ADDRESS.address(),
						amount: AMOUNT
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				EVM_ADDRESS,
				AMOUNT
			),);
		})
	}

	mod erroring_out_when {
		use super::*;

		#[test]
		fn with_zero_balance() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS,
						0
					),
					Error::<Runtime>::InvalidTransferAmount,
				);
			})
		}

		#[test]
		fn with_no_tranche_investor_role() {
			System::externalities().execute_with(|| {
				DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
				Time::mock_now(|| NOW);
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::UnauthorizedTransfer,
				);
			})
		}

		#[test]
		fn without_correct_pool() {
			System::externalities().execute_with(|| {
				DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
				Time::mock_now(|| NOW);
				Permissions::mock_has(move |_, _, _| true);
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::PoolNotFound,
				);
			})
		}

		#[test]
		fn without_correct_tranche_id() {
			System::externalities().execute_with(|| {
				DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
				Time::mock_now(|| NOW);
				Permissions::mock_has(move |_, _, _| true);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::TrancheNotFound,
				);
			})
		}

		#[test]
		fn without_satisfy_lp_filter() {
			System::externalities().execute_with(|| {
				DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
				Time::mock_now(|| NOW);
				Permissions::mock_has(move |_, _, _| true);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);
				TransferFilter::mock_check(|_| Err(DispatchError::Other("Err")));

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS,
						AMOUNT
					),
					DispatchError::Other("Err"),
				);
			})
		}
	}
}

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
