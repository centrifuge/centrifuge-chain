use cfg_primitives::{PoolId, TrancheId};
use cfg_traits::{liquidity_pools::InboundQueue, Millis};
use cfg_types::{
	domain_address::DomainAddress,
	permissions::{PermissionScope, PoolRole, Role},
	tokens::{AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata},
};
use cfg_utils::vec_to_fixed_array;
use frame_support::{
	assert_noop, assert_ok,
	traits::{
		fungibles::{Inspect as _, Mutate as _},
		PalletInfo as _,
	},
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
const NAME: &[u8] = b"Token name";
const SYMBOL: &[u8] = b"Token symbol";
const DECIMALS: u8 = 6;
const TRANCHE_CURRENCY: CurrencyId = CurrencyId::Tranche(POOL_ID, TRANCHE_ID);

mod util {
	use super::*;

	pub fn default_metadata() -> AssetMetadata {
		AssetMetadata {
			decimals: DECIMALS as u32,
			name: Vec::from(NAME).try_into().unwrap(),
			symbol: Vec::from(SYMBOL).try_into().unwrap(),
			..cfg_types::tokens::default_metadata()
		}
	}

	pub fn transferable_metadata() -> AssetMetadata {
		AssetMetadata {
			additional: CustomMetadata {
				transferability: CrossChainTransferability::LiquidityPools,
				..Default::default()
			},
			..default_metadata()
		}
	}

	pub fn wrapped_transferable_metadata() -> AssetMetadata {
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

	pub fn currency_index(currency_id: CurrencyId) -> u128 {
		GeneralCurrencyIndexOf::<Runtime>::try_from(currency_id)
			.unwrap()
			.index
	}
}

mod transfer {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			AssetRegistry::mock_metadata(|_| Some(util::wrapped_transferable_metadata()));
			TransferFilter::mock_check(|_| Ok(()));
			Tokens::mint_into(CURRENCY_ID, &ALICE, AMOUNT).unwrap();
			Gateway::mock_submit(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::Transfer {
						currency: util::currency_index(CURRENCY_ID),
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

			assert_eq!(Tokens::total_issuance(CURRENCY_ID), 0);
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
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

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
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

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
		fn with_wrong_location() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::transferable_metadata()));

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
		fn with_wrong_domain() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::wrapped_transferable_metadata()));

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
		fn without_satisfy_filter() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::wrapped_transferable_metadata()));
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
				AssetRegistry::mock_metadata(|_| Some(util::wrapped_transferable_metadata()));
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
			Tokens::mint_into(TRANCHE_CURRENCY, &ALICE, AMOUNT).unwrap();
			Gateway::mock_submit(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::TransferTrancheTokens {
						pool_id: POOL_ID,
						tranche_id: TRANCHE_ID,
						sender: ALICE.into(),
						domain: EVM_ADDRESS.domain().into(),
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
			));

			let destination = EVM_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &ALICE), 0);
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &destination), AMOUNT);
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
		fn with_wrong_permissions() {
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
		fn with_wrong_pool() {
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
		fn with_wrong_tranche() {
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
		fn without_satisfy_filter() {
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

mod add_pool {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			Permissions::mock_has(move |scope, who, role| {
				assert_eq!(who, ALICE);
				assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
				assert!(matches!(role, Role::PoolRole(PoolRole::PoolAdmin)));
				true
			});
			Pools::mock_pool_exists(|_| true);
			Gateway::mock_submit(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_ADDRESS.domain());
				assert_eq!(msg, Message::AddPool { pool_id: POOL_ID });
				Ok(())
			});

			assert_ok!(LiquidityPools::add_pool(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				EVM_ADDRESS.domain(),
			));
		})
	}

	mod erroring_out_when {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::add_pool(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						EVM_ADDRESS.domain(),
					),
					Error::<Runtime>::PoolNotFound
				);
			})
		}

		#[test]
		fn with_wrong_permissions() {
			System::externalities().execute_with(|| {
				Pools::mock_pool_exists(|_| true);
				Permissions::mock_has(move |_, _, _| false);

				assert_noop!(
					LiquidityPools::add_pool(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						EVM_ADDRESS.domain(),
					),
					Error::<Runtime>::NotPoolAdmin
				);
			})
		}
	}
}

mod add_tranche {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			Permissions::mock_has(move |scope, who, role| {
				assert_eq!(who, ALICE);
				assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
				assert!(matches!(role, Role::PoolRole(PoolRole::PoolAdmin)));
				true
			});
			Pools::mock_pool_exists(|_| true);
			Pools::mock_tranche_exists(|_, _| true);
			AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
			Gateway::mock_submit(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::AddTranche {
						pool_id: POOL_ID,
						tranche_id: TRANCHE_ID,
						token_name: vec_to_fixed_array(NAME),
						token_symbol: vec_to_fixed_array(SYMBOL),
						decimals: DECIMALS,
						restriction_set: 1
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::add_tranche(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				EVM_ADDRESS.domain(),
			));
		})
	}

	mod erroring_out_when {
		use super::*;

		#[test]
		fn with_wrong_permissions() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| false);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS.domain(),
					),
					Error::<Runtime>::NotPoolAdmin
				);
			})
		}

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS.domain(),
					),
					Error::<Runtime>::PoolNotFound
				);
			})
		}

		#[test]
		fn with_wrong_tranche() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS.domain(),
					),
					Error::<Runtime>::TrancheNotFound,
				);
			})
		}

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);
				AssetRegistry::mock_metadata(|_| None);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_ADDRESS.domain(),
					),
					Error::<Runtime>::TrancheMetadataNotFound,
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
