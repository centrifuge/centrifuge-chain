use cfg_traits::Seconds;
use cfg_types::{
	domain_address::DomainAddress,
	permissions::{PermissionScope, PoolRole, Role},
	tokens::CurrencyId,
};
use cfg_utils::vec_to_fixed_array;
use frame_support::{
	assert_noop, assert_ok,
	traits::fungibles::{Inspect as _, Mutate as _},
};
use sp_runtime::{traits::Saturating, DispatchError, TokenError};

use crate::{mock::*, Error, Message, UpdateRestrictionMessage};

mod inbound;

mod transfer {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));
			TransferFilter::mock_check(|_| Ok(()));
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::TransferAssets {
						currency: util::currency_index(CURRENCY_ID),
						receiver: EVM_DOMAIN_ADDRESS.address(),
						amount: AMOUNT
					}
				);
				Ok(())
			});

			Tokens::mint_into(CURRENCY_ID, &ALICE, AMOUNT).unwrap();

			assert_ok!(LiquidityPools::transfer(
				RuntimeOrigin::signed(ALICE),
				CurrencyId::ForeignAsset(1),
				EVM_DOMAIN_ADDRESS,
				AMOUNT
			));

			assert_eq!(Tokens::total_issuance(CURRENCY_ID), 0);
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_zero_balance() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_DOMAIN_ADDRESS,
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
						EVM_DOMAIN_ADDRESS,
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
						EVM_DOMAIN_ADDRESS,
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
						EVM_DOMAIN_ADDRESS,
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
						EVM_DOMAIN_ADDRESS,
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
						EVM_DOMAIN_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::AssetNotLiquidityPoolsWrappedToken
				);
			})
		}

		#[test]
		fn with_wrong_domain() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));

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
				AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));
				TransferFilter::mock_check(|_| Err(DispatchError::Other("Err")));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_DOMAIN_ADDRESS,
						AMOUNT
					),
					DispatchError::Other("Err"),
				);
			})
		}

		#[test]
		fn without_sufficient_balance() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));
				TransferFilter::mock_check(|_| Ok(()));

				assert_noop!(
					LiquidityPools::transfer(
						RuntimeOrigin::signed(ALICE),
						CURRENCY_ID,
						EVM_DOMAIN_ADDRESS,
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

	fn config_mocks() {
		DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
		Time::mock_now(|| NOW);
		Permissions::mock_has(move |scope, who, role| {
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			assert_eq!(who, CONTRACT_ACCOUNT_ID);
			match role {
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, validity)) => {
					assert_eq!(tranche_id, TRANCHE_ID);
					assert_eq!(validity, NOW_SECS);
					true
				}
				Role::PoolRole(PoolRole::FrozenTrancheInvestor(tranche_id)) => {
					assert_eq!(tranche_id, TRANCHE_ID);
					// Default mock has unfrozen investor
					false
				}
				_ => false,
			}
		});
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		TransferFilter::mock_check(|_| Ok(()));
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::TransferTrancheTokens {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					domain: EVM_DOMAIN_ADDRESS.domain().into(),
					receiver: EVM_DOMAIN_ADDRESS.address(),
					amount: AMOUNT
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			Tokens::mint_into(TRANCHE_CURRENCY, &ALICE, AMOUNT).unwrap();

			assert_ok!(LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				EVM_DOMAIN_ADDRESS,
				AMOUNT
			));

			let destination = EVM_DOMAIN_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &ALICE), 0);
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &destination), AMOUNT);
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_zero_balance() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						0
					),
					Error::<Runtime>::InvalidTransferAmount,
				);
			})
		}

		#[test]
		fn with_missing_investor_permissions() {
			System::externalities().execute_with(|| {
				config_mocks();
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::UnauthorizedTransfer,
				);
			})
		}

		#[test]
		fn with_frozen_investor_permissions() {
			System::externalities().execute_with(|| {
				config_mocks();
				Permissions::mock_has(move |scope, who, role| {
					assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
					assert_eq!(who, CONTRACT_ACCOUNT_ID);
					match role {
						Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, validity)) => {
							assert_eq!(tranche_id, TRANCHE_ID);
							assert_eq!(validity, NOW_SECS);
							true
						}
						Role::PoolRole(PoolRole::FrozenTrancheInvestor(tranche_id)) => {
							assert_eq!(tranche_id, TRANCHE_ID);
							true
						}
						_ => false,
					}
				});

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::InvestorDomainAddressFrozen,
				);
			})
		}

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::PoolNotFound,
				);
			})
		}

		#[test]
		fn with_wrong_tranche() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						AMOUNT
					),
					Error::<Runtime>::TrancheNotFound,
				);
			})
		}

		#[test]
		fn without_satisfy_filter() {
			System::externalities().execute_with(|| {
				config_mocks();
				TransferFilter::mock_check(|_| Err(DispatchError::Other("Err")));

				assert_noop!(
					LiquidityPools::transfer_tranche_tokens(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
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
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(msg, Message::AddPool { pool_id: POOL_ID });
				Ok(())
			});

			assert_ok!(LiquidityPools::add_pool(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				EVM_DOMAIN_ADDRESS.domain(),
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::add_pool(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						EVM_DOMAIN_ADDRESS.domain(),
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
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::NotPoolAdmin
				);
			})
		}
	}
}

mod add_tranche {
	use super::*;

	fn config_mocks() {
		let mut hook = [0; 32];
		hook[0..20].copy_from_slice(&DOMAIN_HOOK_ADDRESS_20);
		hook[20..28].copy_from_slice(&1u64.to_be_bytes());
		hook[28..31].copy_from_slice(b"EVM");

		Permissions::mock_has(move |scope, who, role| {
			assert_eq!(who, ALICE);
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			assert!(matches!(role, Role::PoolRole(PoolRole::PoolAdmin)));
			true
		});
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		Gateway::mock_get(move |domain| {
			assert_eq!(domain, &EVM_DOMAIN_ADDRESS.domain());
			Some(DOMAIN_HOOK_ADDRESS_20)
		});
		DomainAddressToAccountId::mock_convert(move |domain| {
			assert_eq!(domain, DomainAddress::EVM(CHAIN_ID, DOMAIN_HOOK_ADDRESS_20));
			hook.clone().into()
		});
		Gateway::mock_handle(move |sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::AddTranche {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					token_name: vec_to_fixed_array(NAME),
					token_symbol: vec_to_fixed_array(SYMBOL),
					decimals: DECIMALS,
					hook,
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::add_tranche(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				EVM_DOMAIN_ADDRESS.domain(),
			));
		})
	}

	mod erroring_out {
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
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::NotPoolAdmin
				);
			})
		}

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::PoolNotFound
				);
			})
		}

		#[test]
		fn with_wrong_tranche() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::TrancheNotFound,
				);
			})
		}

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				config_mocks();
				AssetRegistry::mock_metadata(|_| None);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::TrancheMetadataNotFound,
				);
			})
		}

		#[test]
		fn with_no_hook_address() {
			System::externalities().execute_with(|| {
				config_mocks();
				Gateway::mock_get(|_| None);

				assert_noop!(
					LiquidityPools::add_tranche(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::DomainHookAddressNotFound,
				);
			})
		}
	}
}

mod update_tranche_token_metadata {
	use super::*;

	fn config_mocks() {
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::UpdateTrancheMetadata {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					token_name: vec_to_fixed_array(NAME),
					token_symbol: vec_to_fixed_array(SYMBOL),
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::update_tranche_token_metadata(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				EVM_DOMAIN_ADDRESS.domain(),
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::update_tranche_token_metadata(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::PoolNotFound
				);
			})
		}

		#[test]
		fn with_wrong_tranche() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::update_tranche_token_metadata(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::TrancheNotFound,
				);
			})
		}

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				config_mocks();
				AssetRegistry::mock_metadata(|_| None);

				assert_noop!(
					LiquidityPools::update_tranche_token_metadata(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::TrancheMetadataNotFound,
				);
			})
		}
	}
}

mod update_token_price {
	use super::*;

	fn config_mocks() {
		Pools::mock_get_price(|_, _| Some((TRANCHE_TOKEN_PRICE, 1234)));
		Pools::mock_currency_for(|_| Some(POOL_CURRENCY_ID));
		MarketRatio::mock_market_ratio(|target, origin| {
			assert_eq!(target, CURRENCY_ID);
			assert_eq!(origin, POOL_CURRENCY_ID);
			Ok(MARKET_RATIO)
		});
		AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::UpdateTranchePrice {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					currency: util::currency_index(CURRENCY_ID),
					price: TRANCHE_TOKEN_PRICE
						.saturating_mul(MARKET_RATIO)
						.into_inner(),
					computed_at: 1234
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::update_token_price(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				CURRENCY_ID,
				EVM_DOMAIN_ADDRESS.domain(),
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_missing_tranche_price() {
			System::externalities().execute_with(|| {
				Pools::mock_get_price(|_, _| None);

				assert_noop!(
					LiquidityPools::update_token_price(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						CURRENCY_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::MissingTranchePrice,
				);
			})
		}

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_currency_for(|_| None);

				assert_noop!(
					LiquidityPools::update_token_price(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						CURRENCY_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::PoolNotFound,
				);
			})
		}

		#[test]
		fn with_no_market_ratio() {
			System::externalities().execute_with(|| {
				config_mocks();
				MarketRatio::mock_market_ratio(|_, _| Err(DispatchError::Other("")));

				assert_noop!(
					LiquidityPools::update_token_price(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						CURRENCY_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					DispatchError::Other("")
				);
			})
		}

		#[test]
		fn with_no_transferible_asset() {
			System::externalities().execute_with(|| {
				config_mocks();
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

				assert_noop!(
					LiquidityPools::update_token_price(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						CURRENCY_ID,
						EVM_DOMAIN_ADDRESS.domain(),
					),
					Error::<Runtime>::AssetNotLiquidityPoolsTransferable,
				);
			})
		}
	}
}

mod update_member {
	use super::*;

	const VALID_UNTIL_SECS: Seconds = NOW_SECS + 1;

	fn config_mocks() {
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		Time::mock_now(|| NOW);
		DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
		Permissions::mock_has(move |scope, who, role| {
			assert_eq!(who, CONTRACT_ACCOUNT_ID);
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			assert!(matches!(
				role,
				Role::PoolRole(PoolRole::TrancheInvestor(TRANCHE_ID, VALID_UNTIL_SECS))
			));
			true
		});
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::UpdateRestriction {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					update: UpdateRestrictionMessage::UpdateMember {
						valid_until: VALID_UNTIL_SECS,
						member: EVM_DOMAIN_ADDRESS.address(),
					}
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::update_member(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				EVM_DOMAIN_ADDRESS,
				VALID_UNTIL_SECS,
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::update_member(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						VALID_UNTIL_SECS,
					),
					Error::<Runtime>::PoolNotFound,
				);
			})
		}

		#[test]
		fn with_wrong_tranche() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::update_member(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						VALID_UNTIL_SECS,
					),
					Error::<Runtime>::TrancheNotFound,
				);
			})
		}

		#[test]
		fn with_wrong_time() {
			System::externalities().execute_with(|| {
				config_mocks();
				Time::mock_now(|| VALID_UNTIL_SECS * 1000);

				assert_noop!(
					LiquidityPools::update_member(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						VALID_UNTIL_SECS,
					),
					Error::<Runtime>::InvalidTrancheInvestorValidity,
				);
			})
		}

		#[test]
		fn with_wrong_permissions() {
			System::externalities().execute_with(|| {
				config_mocks();
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::update_member(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN_ADDRESS,
						VALID_UNTIL_SECS,
					),
					Error::<Runtime>::InvestorDomainAddressNotAMember,
				);
			})
		}
	}
}

mod add_currency {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::AddAsset {
						currency: util::currency_index(CURRENCY_ID),
						evm_address: CONTRACT_ACCOUNT,
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::add_currency(
				RuntimeOrigin::signed(ALICE),
				CURRENCY_ID
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| None);

				assert_noop!(
					LiquidityPools::add_currency(RuntimeOrigin::signed(ALICE), CURRENCY_ID),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}

		#[test]
		fn with_unsupported_token() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

				assert_noop!(
					LiquidityPools::add_currency(RuntimeOrigin::signed(ALICE), CurrencyId::Native),
					TokenError::Unsupported,
				);
			})
		}

		#[test]
		fn with_no_transferible_asset() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

				assert_noop!(
					LiquidityPools::add_currency(RuntimeOrigin::signed(ALICE), CURRENCY_ID),
					Error::<Runtime>::AssetNotLiquidityPoolsTransferable,
				);
			})
		}

		#[test]
		fn with_wrong_location() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| Some(util::transferable_metadata()));

				assert_noop!(
					LiquidityPools::add_currency(RuntimeOrigin::signed(ALICE), CURRENCY_ID),
					Error::<Runtime>::AssetNotLiquidityPoolsWrappedToken
				);
			})
		}
	}
}

mod allow_investment_currency {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			AssetRegistry::mock_metadata(|_| Some(util::pool_locatable_transferable_metadata()));
			Permissions::mock_has(move |scope, who, role| {
				assert_eq!(who, ALICE);
				assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
				assert!(matches!(role, Role::PoolRole(PoolRole::PoolAdmin)));
				true
			});
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::AllowAsset {
						pool_id: POOL_ID,
						currency: util::currency_index(CURRENCY_ID),
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::allow_investment_currency(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				CURRENCY_ID
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_permissions() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| false);

				assert_noop!(
					LiquidityPools::allow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::NotPoolAdmin
				);
			})
		}

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| None);

				assert_noop!(
					LiquidityPools::allow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}

		#[test]
		fn with_unsupported_token() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

				assert_noop!(
					LiquidityPools::allow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CurrencyId::Native
					),
					TokenError::Unsupported,
				);
			})
		}

		#[test]
		fn with_no_transferible_asset() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

				assert_noop!(
					LiquidityPools::allow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetNotLiquidityPoolsTransferable,
				);
			})
		}

		#[test]
		fn with_wrong_location() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::transferable_metadata()));

				assert_noop!(
					LiquidityPools::allow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetNotLiquidityPoolsWrappedToken
				);
			})
		}

		#[test]
		fn with_wrong_pool_currency() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));

				assert_noop!(
					LiquidityPools::allow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetMetadataNotPoolCurrency
				);
			})
		}
	}
}

mod disallow_investment_currency {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			AssetRegistry::mock_metadata(|_| Some(util::pool_locatable_transferable_metadata()));
			Permissions::mock_has(move |scope, who, role| {
				assert_eq!(who, ALICE);
				assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
				assert!(matches!(role, Role::PoolRole(PoolRole::PoolAdmin)));
				true
			});
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::DisallowAsset {
						pool_id: POOL_ID,
						currency: util::currency_index(CURRENCY_ID),
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::disallow_investment_currency(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				CURRENCY_ID
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_permissions() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| false);

				assert_noop!(
					LiquidityPools::disallow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::NotPoolAdmin
				);
			})
		}

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| None);

				assert_noop!(
					LiquidityPools::disallow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}

		#[test]
		fn with_unsupported_token() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

				assert_noop!(
					LiquidityPools::disallow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CurrencyId::Native
					),
					TokenError::Unsupported,
				);
			})
		}

		#[test]
		fn with_no_transferible_asset() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

				assert_noop!(
					LiquidityPools::disallow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetNotLiquidityPoolsTransferable,
				);
			})
		}

		#[test]
		fn with_wrong_location() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::transferable_metadata()));

				assert_noop!(
					LiquidityPools::disallow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetNotLiquidityPoolsWrappedToken
				);
			})
		}

		#[test]
		fn with_wrong_pool_currency() {
			System::externalities().execute_with(|| {
				Permissions::mock_has(move |_, _, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::locatable_transferable_metadata()));

				assert_noop!(
					LiquidityPools::disallow_investment_currency(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						CURRENCY_ID
					),
					Error::<Runtime>::AssetMetadataNotPoolCurrency
				);
			})
		}
	}
}

mod schedule_upgrade {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, TreasuryAccount::get());
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::ScheduleUpgrade {
						contract: CONTRACT_ACCOUNT
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::schedule_upgrade(
				RuntimeOrigin::root(),
				CHAIN_ID,
				CONTRACT_ACCOUNT,
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_origin() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::schedule_upgrade(
						RuntimeOrigin::signed(ALICE),
						CHAIN_ID,
						CONTRACT_ACCOUNT,
					),
					DispatchError::BadOrigin
				);
			})
		}
	}
}

mod cancel_upgrade {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, TreasuryAccount::get());
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::CancelUpgrade {
						contract: CONTRACT_ACCOUNT
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::cancel_upgrade(
				RuntimeOrigin::root(),
				CHAIN_ID,
				CONTRACT_ACCOUNT,
			));
		})
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_origin() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::cancel_upgrade(
						RuntimeOrigin::signed(ALICE),
						CHAIN_ID,
						CONTRACT_ACCOUNT,
					),
					DispatchError::BadOrigin
				);
			})
		}
	}
}

mod freeze {
	use sp_runtime::DispatchError;

	use super::*;
	use crate::message::UpdateRestrictionMessage;

	fn config_mocks(receiver: DomainAddress) {
		DomainAccountToDomainAddress::mock_convert(move |_| receiver.clone());
		DomainAddressToAccountId::mock_convert(move |_| ALICE_EVM_LOCAL_ACCOUNT);
		Time::mock_now(|| NOW);
		Permissions::mock_has(move |scope, who, role| {
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			match role {
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, validity)) => {
					assert_eq!(who, ALICE_EVM_LOCAL_ACCOUNT);
					assert_eq!(tranche_id, TRANCHE_ID);
					assert_eq!(validity, NOW_SECS);
					true
				}
				Role::PoolRole(PoolRole::FrozenTrancheInvestor(tranche_id)) => {
					assert_eq!(who, ALICE_EVM_LOCAL_ACCOUNT);
					assert_eq!(tranche_id, TRANCHE_ID);
					// Default mock has frozen investor
					true
				}
				Role::PoolRole(PoolRole::PoolAdmin) => {
					assert_eq!(who, ALICE);
					true
				}
				_ => false,
			}
		});
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, ALICE_EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::UpdateRestriction {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					update: UpdateRestrictionMessage::Freeze {
						address: ALICE_EVM_DOMAIN_ADDRESS.address().into()
					}
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks(ALICE_EVM_DOMAIN_ADDRESS);

			assert_ok!(LiquidityPools::freeze_investor(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				ALICE_EVM_DOMAIN_ADDRESS
			));
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_bad_origin_unsigned_none() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::freeze_investor(
						RuntimeOrigin::none(),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					DispatchError::BadOrigin
				);
			});
		}
		#[test]
		fn with_bad_origin_unsigned_root() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::freeze_investor(
						RuntimeOrigin::root(),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					DispatchError::BadOrigin
				);
			});
		}

		#[test]
		fn with_pool_dne() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::freeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::PoolNotFound
				);
			});
		}

		#[test]
		fn with_tranche_dne() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::freeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::TrancheNotFound
				);
			});
		}

		#[test]
		fn with_origin_not_admin() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::freeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::NotPoolAdmin
				);
			});
		}

		#[test]
		fn with_investor_not_member() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Permissions::mock_has(move |scope, who, role| {
					assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
					match role {
						Role::PoolRole(PoolRole::PoolAdmin) => {
							assert_eq!(who, ALICE);
							true
						}
						_ => false,
					}
				});

				assert_noop!(
					LiquidityPools::freeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::InvestorDomainAddressNotAMember
				);
			});
		}

		#[test]
		fn with_investor_frozen() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Permissions::mock_has(move |scope, who, role| {
					assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
					match role {
						Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, validity)) => {
							assert_eq!(who, ALICE_EVM_LOCAL_ACCOUNT);
							assert_eq!(tranche_id, TRANCHE_ID);
							assert_eq!(validity, NOW_SECS);
							true
						}
						Role::PoolRole(PoolRole::FrozenTrancheInvestor(tranche_id)) => {
							assert_eq!(who, ALICE_EVM_LOCAL_ACCOUNT);
							assert_eq!(tranche_id, TRANCHE_ID);
							false
						}
						Role::PoolRole(PoolRole::PoolAdmin) => {
							assert_eq!(who, ALICE);
							true
						}
						_ => false,
					}
				});

				assert_noop!(
					LiquidityPools::freeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::InvestorDomainAddressFrozen
				);
			});
		}
	}
}

mod unfreeze {
	use sp_runtime::DispatchError;

	use super::*;
	use crate::message::UpdateRestrictionMessage;

	fn config_mocks(receiver: DomainAddress) {
		DomainAccountToDomainAddress::mock_convert(move |_| receiver.clone());
		DomainAddressToAccountId::mock_convert(move |_| ALICE_EVM_LOCAL_ACCOUNT);
		Time::mock_now(|| NOW);
		Permissions::mock_has(move |scope, who, role| {
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			match role {
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, validity)) => {
					assert_eq!(who, ALICE_EVM_LOCAL_ACCOUNT);
					assert_eq!(tranche_id, TRANCHE_ID);
					assert_eq!(validity, NOW_SECS);
					true
				}
				Role::PoolRole(PoolRole::FrozenTrancheInvestor(tranche_id)) => {
					assert_eq!(who, ALICE_EVM_LOCAL_ACCOUNT);
					assert_eq!(tranche_id, TRANCHE_ID);
					// Default mock has unfrozen investor
					false
				}
				Role::PoolRole(PoolRole::PoolAdmin) => {
					assert_eq!(who, ALICE);
					true
				}
				_ => false,
			}
		});
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, ALICE_EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::UpdateRestriction {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					update: UpdateRestrictionMessage::Unfreeze {
						address: ALICE_EVM_DOMAIN_ADDRESS.address().into()
					}
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks(ALICE_EVM_DOMAIN_ADDRESS);

			assert_ok!(LiquidityPools::unfreeze_investor(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				ALICE_EVM_DOMAIN_ADDRESS
			));
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_bad_origin_unsigned_none() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::unfreeze_investor(
						RuntimeOrigin::none(),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					DispatchError::BadOrigin
				);
			});
		}
		#[test]
		fn with_bad_origin_unsigned_root() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::unfreeze_investor(
						RuntimeOrigin::root(),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					DispatchError::BadOrigin
				);
			});
		}

		#[test]
		fn with_pool_dne() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::unfreeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::PoolNotFound
				);
			});
		}

		#[test]
		fn with_tranche_dne() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::unfreeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::TrancheNotFound
				);
			});
		}

		#[test]
		fn with_origin_not_admin() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::unfreeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::NotPoolAdmin
				);
			});
		}

		#[test]
		fn with_investor_not_member() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Permissions::mock_has(move |scope, who, role| {
					assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
					match role {
						Role::PoolRole(PoolRole::PoolAdmin) => {
							assert_eq!(who, ALICE);
							true
						}
						_ => false,
					}
				});

				assert_noop!(
					LiquidityPools::unfreeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::InvestorDomainAddressNotAMember
				);
			});
		}

		#[test]
		fn with_investor_unfrozen() {
			System::externalities().execute_with(|| {
				config_mocks(ALICE_EVM_DOMAIN_ADDRESS);
				Permissions::mock_has(move |scope, who, role| {
					assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
					match role {
						Role::PoolRole(PoolRole::FrozenTrancheInvestor(tranche_id)) => {
							assert_eq!(who, ALICE_EVM_LOCAL_ACCOUNT);
							assert_eq!(tranche_id, TRANCHE_ID);
							true
						}
						_ => true,
					}
				});

				assert_noop!(
					LiquidityPools::unfreeze_investor(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						ALICE_EVM_DOMAIN_ADDRESS
					),
					Error::<Runtime>::InvestorDomainAddressFrozen
				);
			});
		}
	}
}

mod update_tranche_hook {
	use super::*;

	fn config_mocks() {
		DomainAddressToAccountId::mock_convert(move |_| DOMAIN_HOOK_ADDRESS_32.into());
		Permissions::mock_has(move |scope, who, role| {
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			match role {
				Role::PoolRole(PoolRole::PoolAdmin) => {
					assert_eq!(who, ALICE);
					true
				}
				_ => false,
			}
		});
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, ALICE);
			assert_eq!(destination, EVM_DOMAIN);
			assert_eq!(
				msg,
				Message::UpdateTrancheHook {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					hook: DOMAIN_HOOK_ADDRESS_32
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::update_tranche_hook(
				RuntimeOrigin::signed(ALICE),
				POOL_ID,
				TRANCHE_ID,
				EVM_DOMAIN,
				DOMAIN_HOOK_ADDRESS_20
			));
		});
	}

	mod erroring_out {
		use cfg_types::domain_address::Domain;

		use super::*;

		#[test]
		fn with_bad_origin_unsigned_none() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::update_tranche_hook(
						RuntimeOrigin::none(),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN,
						DOMAIN_HOOK_ADDRESS_20
					),
					DispatchError::BadOrigin
				);
			});
		}
		#[test]
		fn with_bad_origin_unsigned_root() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::update_tranche_hook(
						RuntimeOrigin::root(),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN,
						DOMAIN_HOOK_ADDRESS_20
					),
					DispatchError::BadOrigin
				);
			});
		}

		#[test]
		fn with_pool_dne() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::update_tranche_hook(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN,
						DOMAIN_HOOK_ADDRESS_20
					),
					Error::<Runtime>::PoolNotFound
				);
			});
		}

		#[test]
		fn with_tranche_dne() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::update_tranche_hook(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN,
						DOMAIN_HOOK_ADDRESS_20
					),
					Error::<Runtime>::TrancheNotFound
				);
			});
		}

		#[test]
		fn with_origin_not_admin() {
			System::externalities().execute_with(|| {
				config_mocks();
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::update_tranche_hook(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						EVM_DOMAIN,
						DOMAIN_HOOK_ADDRESS_20
					),
					Error::<Runtime>::NotPoolAdmin
				);
			});
		}

		#[test]
		fn with_invalid_domain() {
			System::externalities().execute_with(|| {
				config_mocks();

				assert_noop!(
					LiquidityPools::update_tranche_hook(
						RuntimeOrigin::signed(ALICE),
						POOL_ID,
						TRANCHE_ID,
						Domain::Centrifuge,
						DOMAIN_HOOK_ADDRESS_20
					),
					Error::<Runtime>::InvalidDomain
				);
			});
		}
	}
}

mod recover_assets {
	use super::*;

	const CONTRACT: [u8; 32] = [42; 32];
	const ASSET: [u8; 32] = [43; 32];

	fn config_mocks() {
		DomainAddressToAccountId::mock_convert(move |_| ALICE_EVM_LOCAL_ACCOUNT);
		Permissions::mock_has(|_, _, _| false);
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, TreasuryAccount::get());
			assert_eq!(destination, EVM_DOMAIN);
			assert_eq!(
				msg,
				Message::RecoverAssets {
					contract: CONTRACT,
					asset: ASSET,
					recipient: ALICE_EVM_LOCAL_ACCOUNT.into(),
					amount: sp_core::U256::from(AMOUNT).into(),
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::recover_assets(
				RuntimeOrigin::root(),
				ALICE_EVM_DOMAIN_ADDRESS,
				CONTRACT,
				ASSET,
				AMOUNT.into(),
			));
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_origin_none() {
			System::externalities().execute_with(|| {
				config_mocks();

				assert_noop!(
					LiquidityPools::recover_assets(
						RuntimeOrigin::none(),
						ALICE_EVM_DOMAIN_ADDRESS,
						CONTRACT,
						ASSET,
						AMOUNT.into(),
					),
					DispatchError::BadOrigin
				);
			});
		}

		#[test]
		fn with_wrong_origin_signed() {
			System::externalities().execute_with(|| {
				config_mocks();

				assert_noop!(
					LiquidityPools::recover_assets(
						RuntimeOrigin::signed(ALICE.into()),
						ALICE_EVM_DOMAIN_ADDRESS,
						CONTRACT,
						ASSET,
						AMOUNT.into(),
					),
					DispatchError::BadOrigin
				);
			});
		}

		#[test]
		fn with_wrong_domain() {
			System::externalities().execute_with(|| {
				config_mocks();

				assert_noop!(
					LiquidityPools::recover_assets(
						RuntimeOrigin::root(),
						DomainAddress::Centrifuge(ALICE.into()),
						CONTRACT,
						ASSET,
						AMOUNT.into(),
					),
					Error::<Runtime>::InvalidDomain
				);
			});
		}
	}
}
