use cfg_traits::liquidity_pools::InboundQueue;
use cfg_types::{
	domain_address::DomainAddress,
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
			LiquidityPools::submit(EVM_DOMAIN_ADDRESS, msg),
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

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::Transfer {
					currency: util::currency_index(CURRENCY_ID),
					sender: EVM_DOMAIN_ADDRESS.address(),
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::Transfer {
							currency: util::currency_index(CURRENCY_ID),
							sender: EVM_DOMAIN_ADDRESS.address(),
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::Transfer {
							currency: util::currency_index(CURRENCY_ID),
							sender: EVM_DOMAIN_ADDRESS.address(),
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
	use super::*;

	fn config_mocks(receiver: DomainAddress) {
		DomainAccountToDomainAddress::mock_convert(move |_| receiver.clone());
		DomainAddressToAccountId::mock_convert(move |_| ALICE);
		Time::mock_now(|| NOW);
		Permissions::mock_has(move |scope, who, role| {
			assert_eq!(who, ALICE);
			assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
			assert!(matches!(
				role,
				Role::PoolRole(PoolRole::TrancheInvestor(TRANCHE_ID, NOW_SECS))
			));
			true
		});
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
	}

	#[test]
	fn success_with_centrifuge_domain() {
		System::externalities().execute_with(|| {
			config_mocks(CENTRIFUGE_DOMAIN_ADDRESS);

			Tokens::mint_into(
				TRANCHE_CURRENCY,
				&EVM_DOMAIN_ADDRESS.domain().into_account(),
				AMOUNT,
			)
			.unwrap();

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::TransferTrancheTokens {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					sender: EVM_DOMAIN_ADDRESS.address(),
					domain: CENTRIFUGE_DOMAIN_ADDRESS.domain().into(),
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
		System::externalities().execute_with(|| {
			config_mocks(ALICE_EVM_DOMAIN_ADDRESS);

			TransferFilter::mock_check(|_| Ok(()));
			Gateway::mock_submit(|sender, destination, msg| {
				assert_eq!(sender, ALICE);
				assert_eq!(destination, ALICE_EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::TransferTrancheTokens {
						pool_id: POOL_ID,
						tranche_id: TRANCHE_ID,
						sender: ALICE.into(),
						domain: ALICE_EVM_DOMAIN_ADDRESS.domain().into(),
						receiver: ALICE_EVM_DOMAIN_ADDRESS.address().into(),
						amount: AMOUNT
					}
				);
				Ok(())
			});

			Tokens::mint_into(
				TRANCHE_CURRENCY,
				&EVM_DOMAIN_ADDRESS.domain().into_account(),
				AMOUNT,
			)
			.unwrap();

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::TransferTrancheTokens {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					sender: EVM_DOMAIN_ADDRESS.address(),
					domain: ALICE_EVM_DOMAIN_ADDRESS.domain().into(),
					receiver: ALICE.into(),
					amount: AMOUNT
				}
			));

			let origin = EVM_DOMAIN_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &origin), 0);

			let destination = ALICE_EVM_DOMAIN_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &destination), AMOUNT);
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_zero_balance() {
			System::externalities().execute_with(|| {
				config_mocks(CENTRIFUGE_DOMAIN_ADDRESS);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: EVM_DOMAIN_ADDRESS.address(),
							domain: CENTRIFUGE_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: 0,
						}
					),
					Error::<Runtime>::InvalidTransferAmount,
				);
			})
		}

		#[test]
		fn with_wrong_permissions() {
			System::externalities().execute_with(|| {
				config_mocks(CENTRIFUGE_DOMAIN_ADDRESS);
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: EVM_DOMAIN_ADDRESS.address(),
							domain: CENTRIFUGE_DOMAIN_ADDRESS.domain().into(),
							receiver: ALICE.into(),
							amount: AMOUNT,
						}
					),
					Error::<Runtime>::UnauthorizedTransfer,
				);
			})
		}

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				config_mocks(CENTRIFUGE_DOMAIN_ADDRESS);
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: EVM_DOMAIN_ADDRESS.address(),
							domain: CENTRIFUGE_DOMAIN_ADDRESS.domain().into(),
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
				config_mocks(CENTRIFUGE_DOMAIN_ADDRESS);
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: EVM_DOMAIN_ADDRESS.address(),
							domain: CENTRIFUGE_DOMAIN_ADDRESS.domain().into(),
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
				config_mocks(CENTRIFUGE_DOMAIN_ADDRESS);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: EVM_DOMAIN_ADDRESS.address(),
							domain: CENTRIFUGE_DOMAIN_ADDRESS.domain().into(),
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
		DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
		DomainAddressToAccountId::mock_convert(|_| ALICE);
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_increase_foreign_investment(
			|who, investment_id, amount, foreign_currency| {
				assert_eq!(*who, ALICE);
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

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::IncreaseInvestOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::IncreaseInvestOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::IncreaseInvestOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::IncreaseInvestOrder {
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
		DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
		DomainAddressToAccountId::mock_convert(|_| ALICE);
		ForeignInvestment::mock_investment(|_, _| Ok(AMOUNT));
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_decrease_foreign_investment(
			|who, investment_id, amount, foreign_currency| {
				assert_eq!(*who, ALICE);
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

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::CancelInvestOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::CancelInvestOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::CancelInvestOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::CancelInvestOrder {
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
		DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
		DomainAddressToAccountId::mock_convert(|_| ALICE);
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_increase_foreign_redemption(
			|who, investment_id, amount, foreign_currency| {
				assert_eq!(*who, ALICE);
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

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::IncreaseRedeemOrder {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					investor: ALICE.into(),
					currency: util::currency_index(CURRENCY_ID),
					amount: AMOUNT,
				},
			));

			let destination = EVM_DOMAIN_ADDRESS.domain().into_account();
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &destination), 0);
			assert_eq!(Tokens::balance(TRANCHE_CURRENCY, &ALICE), AMOUNT);
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::IncreaseRedeemOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::IncreaseRedeemOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::IncreaseRedeemOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::IncreaseRedeemOrder {
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
		DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
		DomainAddressToAccountId::mock_convert(|_| ALICE);
		ForeignInvestment::mock_redemption(|_, _| Ok(AMOUNT));
		Pools::mock_pool_exists(|_| true);
		Pools::mock_tranche_exists(|_, _| true);
		AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
		ForeignInvestment::mock_decrease_foreign_redemption(
			|who, investment_id, amount, foreign_currency| {
				assert_eq!(*who, ALICE);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(amount, AMOUNT);
				assert_eq!(foreign_currency, CURRENCY_ID);

				// Side effects of this call
				ForeignInvestment::mock_redemption(|_, _| Ok(0));
				Tokens::mint_into(TRANCHE_CURRENCY, &ALICE, AMOUNT).unwrap();
				Ok(())
			},
		);
		Gateway::mock_submit(|sender, destination, msg| {
			assert_eq!(sender, TreasuryAccount::get());
			assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
			assert_eq!(
				msg,
				Message::ExecutedDecreaseRedeemOrder {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					investor: ALICE.into(),
					currency: util::currency_index(CURRENCY_ID),
					tranche_tokens_payout: AMOUNT,
					remaining_redeem_amount: 0,
				}
			);
			Ok(())
		});
	}

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			config_mocks();

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::CancelRedeemOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::CancelRedeemOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::CancelRedeemOrder {
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
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::CancelRedeemOrder {
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
