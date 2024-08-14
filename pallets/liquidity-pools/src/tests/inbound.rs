use cfg_traits::liquidity_pools::InboundMessageHandler;
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	permissions::{PermissionScope, PoolRole, Role},
};
use frame_support::{
	assert_noop, assert_ok,
	traits::fungibles::{Inspect as _, Mutate as _},
};

use crate::{mock::*, Error, Message};

#[test]
fn receiving_invalid_message() {
	System::externalities().execute_with(|| {
		// Add pool is an outbound message, not valid to be received
		let msg = Message::AddPool { pool_id: 123 };

		assert_noop!(
			LiquidityPools::handle(EVM_DOMAIN_ADDRESS, msg),
			Error::<Runtime>::InvalidIncomingMessage,
		);
	})
}

mod handle_transfer {
	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

			assert_ok!(LiquidityPools::handle(
				EVM_DOMAIN_ADDRESS,
				Message::TransferAssets {
					currency: util::currency_index(CURRENCY_ID),
					receiver: ALICE.into(),
					amount: AMOUNT,
				},
			));

			assert_eq!(Tokens::balance(CURRENCY_ID, &ALICE.into()), AMOUNT);
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_zero_balance() {
			System::externalities().execute_with(|| {
				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferAssets {
							currency: util::currency_index(CURRENCY_ID),
							receiver: ALICE.into(),
							amount: 0,
						},
					),
					Error::<Runtime>::InvalidTransferAmount
				);
			})
		}

		#[test]
		fn with_no_metadata() {
			System::externalities().execute_with(|| {
				AssetRegistry::mock_metadata(|_| None);
				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferAssets {
							currency: util::currency_index(CURRENCY_ID),
							receiver: ALICE.into(),
							amount: AMOUNT,
						},
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}
	}
}

mod handle_tranche_tokens_transfer {
	use cfg_types::domain_address::Domain;

	use super::*;

	fn config_mocks(receiver: DomainAddress) {
		Time::mock_now(|| NOW);
		Permissions::mock_has(move |scope, who, role| {
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			assert_eq!(who, receiver.as_local());
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
	}

	#[test]
	fn success_with_centrifuge_domain() {
		System::externalities().execute_with(|| {
			config_mocks(LOCAL_DOMAIN_ADDRESS);

			Tokens::mint_into(
				TRANCHE_CURRENCY,
				&EVM_DOMAIN_ADDRESS.domain().into_account(),
				AMOUNT,
			)
			.unwrap();

			assert_ok!(LiquidityPools::handle(
				EVM_DOMAIN_ADDRESS,
				Message::TransferTrancheTokens {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					domain: Domain::Local.into(),
					receiver: ALICE.into(),
					amount: AMOUNT
				}
			));

			let origin = EVM_DOMAIN_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &origin), 0);
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &ALICE.into()), AMOUNT);
		});
	}

	#[test]
	fn success_with_evm_domain() {
		const OTHER_CHAIN_ID: u64 = CHAIN_ID + 1;
		const OTHER_DOMAIN: Domain = Domain::Evm(OTHER_CHAIN_ID);
		const OTHER_DOMAIN_ADDRESS_ALICE: DomainAddress =
			DomainAddress::Evm(OTHER_CHAIN_ID, ALICE_ETH);

		System::externalities().execute_with(|| {
			config_mocks(OTHER_DOMAIN_ADDRESS_ALICE);

			TransferFilter::mock_check(|_| Ok(()));
			Gateway::mock_handle(|sender, destination, msg| {
				assert_eq!(sender, OTHER_DOMAIN_ADDRESS_ALICE.as_local());
				assert_eq!(destination, OTHER_DOMAIN);
				assert_eq!(
					msg,
					Message::TransferTrancheTokens {
						pool_id: POOL_ID,
						tranche_id: TRANCHE_ID,
						domain: OTHER_DOMAIN.into(),
						receiver: OTHER_DOMAIN_ADDRESS_ALICE.as_local(),
						amount: AMOUNT
					}
				);
				Ok(())
			});

			let origin = EVM_DOMAIN.into_account();
			Tokens::mint_into(TRANCHE_CURRENCY, &origin, AMOUNT).unwrap();

			assert_ok!(LiquidityPools::handle(
				EVM_DOMAIN_ADDRESS,
				Message::TransferTrancheTokens {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					domain: OTHER_DOMAIN.into(),
					receiver: ALICE.into(),
					amount: AMOUNT
				}
			));

			let destination = OTHER_DOMAIN.into_account();
			assert_ne!(destination, origin);
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &origin), 0);
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &destination), AMOUNT);
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_zero_balance() {
			System::externalities().execute_with(|| {
				config_mocks(LOCAL_DOMAIN_ADDRESS);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							domain: LOCAL_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: 0,
						}
					),
					Error::<Runtime>::InvalidTransferAmount,
				);
			})
		}

		#[test]
		fn without_investor_permissions() {
			System::externalities().execute_with(|| {
				config_mocks(LOCAL_DOMAIN_ADDRESS);
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							domain: LOCAL_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: AMOUNT,
						}
					),
					Error::<Runtime>::UnauthorizedTransfer,
				);
			})
		}

		#[test]
		fn inbound_with_frozen_investor_permissions() {
			System::externalities().execute_with(|| {
				config_mocks(LOCAL_DOMAIN_ADDRESS);
				Permissions::mock_has(|_, _, _| true);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							domain: LOCAL_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: AMOUNT,
						}
					),
					Error::<Runtime>::InvestorDomainAddressFrozen,
				);
			})
		}

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks(LOCAL_DOMAIN_ADDRESS);
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							domain: LOCAL_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: AMOUNT,
						}
					),
					Error::<Runtime>::PoolNotFound,
				);
			})
		}

		#[test]
		fn with_wrong_tranche() {
			System::externalities().execute_with(|| {
				config_mocks(LOCAL_DOMAIN_ADDRESS);
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							domain: LOCAL_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: AMOUNT,
						}
					),
					Error::<Runtime>::TrancheNotFound,
				);
			})
		}

		#[test]
		fn without_sufficient_balance() {
			System::externalities().execute_with(|| {
				config_mocks(LOCAL_DOMAIN_ADDRESS);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							domain: LOCAL_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: AMOUNT,
						}
					),
					orml_tokens::Error::<Runtime>::BalanceTooLow
				);
			})
		}
	}
}

mod handle_increase_invest_order {
	use super::*;

	fn config_mocks() {
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_increase_foreign_investment(
			|who, investment_id, amount, foreign_currency| {
				assert_eq!(*who, DomainAddress::new(EVM_DOMAIN, ALICE_32).as_local());
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(amount, AMOUNT);
				assert_eq!(foreign_currency, CURRENCY_ID);
				Ok(())
			},
		);
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::handle(
				EVM_DOMAIN_ADDRESS,
				Message::DepositRequest {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					investor: ALICE.into(),
					currency: util::currency_index(CURRENCY_ID),
					amount: AMOUNT,
				},
			));
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::DepositRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
							amount: AMOUNT,
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::DepositRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
							amount: AMOUNT,
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::DepositRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
							amount: AMOUNT,
						},
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}
	}
}

mod handle_cancel_invest_order {
	use super::*;

	fn config_mocks() {
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_cancel_foreign_investment(
			|who, investment_id, foreign_currency| {
				assert_eq!(*who, DomainAddress::new(EVM_DOMAIN, ALICE_32).as_local());
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(foreign_currency, CURRENCY_ID);
				Ok(())
			},
		);
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::handle(
				EVM_DOMAIN_ADDRESS,
				Message::CancelDepositRequest {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					investor: ALICE.into(),
					currency: util::currency_index(CURRENCY_ID),
				},
			));
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::CancelDepositRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::CancelDepositRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::CancelDepositRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
						},
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}
	}
}

mod handle_increase_redeem_order {
	use super::*;

	fn config_mocks() {
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_increase_foreign_redemption(
			|who, investment_id, amount, foreign_currency| {
				assert_eq!(*who, DomainAddress::new(EVM_DOMAIN, ALICE_32).as_local());
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(amount, AMOUNT);
				assert_eq!(foreign_currency, CURRENCY_ID);
				Ok(())
			},
		);
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			Tokens::mint_into(
				TRANCHE_CURRENCY,
				&EVM_DOMAIN_ADDRESS.domain().into_account(),
				AMOUNT,
			)
			.unwrap();

			assert_ok!(LiquidityPools::handle(
				EVM_DOMAIN_ADDRESS,
				Message::RedeemRequest {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					investor: ALICE.into(),
					currency: util::currency_index(CURRENCY_ID),
					amount: AMOUNT,
				},
			));

			let destination = EVM_DOMAIN_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &destination), 0);
			let local = DomainAddress::new(EVM_DOMAIN, ALICE_32).as_local();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &local), AMOUNT);
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::RedeemRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
							amount: AMOUNT,
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::RedeemRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
							amount: AMOUNT,
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::RedeemRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
							amount: AMOUNT,
						},
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}

		#[test]
		fn without_sufficient_balance() {
			System::externalities().execute_with(|| {
				config_mocks();

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::RedeemRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
							amount: AMOUNT,
						},
					),
					orml_tokens::Error::<Runtime>::BalanceTooLow
				);
			})
		}
	}
}

mod handle_cancel_redeem_order {
	use super::*;

	fn config_mocks() {
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_cancel_foreign_redemption(
			|who, investment_id, foreign_currency| {
				let local_who = DomainAddress::new(EVM_DOMAIN, ALICE_32).as_local();
				assert_eq!(*who, local_who);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(foreign_currency, CURRENCY_ID);

				// Side effects of this call
				Tokens::mint_into(TRANCHE_CURRENCY, &local_who, AMOUNT).unwrap();
				Ok(AMOUNT)
			},
		);
		Gateway::mock_handle(|sender, destination, msg| {
			assert_eq!(sender, TreasuryAccount::get());
			assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::FulfilledCancelRedeemRequest {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					investor: DomainAddress::new(EVM_DOMAIN, ALICE_32).as_local(),
					currency: util::currency_index(CURRENCY_ID),
					tranche_tokens_payout: AMOUNT,
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::handle(
				EVM_DOMAIN_ADDRESS,
				Message::CancelRedeemRequest {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					investor: ALICE.into(),
					currency: util::currency_index(CURRENCY_ID),
				},
			));

			let destination = EVM_DOMAIN_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &ALICE), 0);
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &destination), AMOUNT);
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks();
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::CancelRedeemRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::CancelRedeemRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
						},
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
					LiquidityPools::handle(
						EVM_DOMAIN_ADDRESS,
						Message::CancelRedeemRequest {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							investor: ALICE.into(),
							currency: util::currency_index(CURRENCY_ID),
						},
					),
					Error::<Runtime>::AssetNotFound,
				);
			})
		}
	}
}
