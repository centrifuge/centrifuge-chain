use cfg_traits::liquidity_pools::InboundQueue;
use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok, traits::fungibles::Mutate as _};

use crate::{mock::*, Error, Message};

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
					sender: ALICE.into(),
					receiver: EVM_DOMAIN_ADDRESS.address(),
					amount: AMOUNT,
				},
			));
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
							sender: ALICE.into(),
							receiver: EVM_DOMAIN_ADDRESS.address(),
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
							sender: ALICE.into(),
							receiver: EVM_DOMAIN_ADDRESS.address(),
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

	#[test]
	fn success_with_centrifuge_domain() {
		System::externalities().execute_with(|| {
			DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
			DomainAddressToAccountId::mock_convert(|_| ALICE);
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
			Tokens::mint_into(
				TRANCHE_CURRENCY,
				&EVM_DOMAIN_ADDRESS.domain().into_account(),
				AMOUNT * 2,
			)
			.unwrap();

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::TransferTrancheTokens {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					sender: ALICE.into(),
					domain: CENTRIFUGE_DOMAIN_ADDRESS.domain().into(),
					receiver: CENTRIFUGE_DOMAIN_ADDRESS.address(),
					amount: AMOUNT
				}
			));
		});
	}

	#[test]
	fn success_with_evm_domain() {
		System::externalities().execute_with(|| {
			DomainAccountToDomainAddress::mock_convert(|_| EVM_DOMAIN_ADDRESS);
			DomainAddressToAccountId::mock_convert(|_| CONTRACT_ACCOUNT_ID);
			Time::mock_now(|| NOW);
			Permissions::mock_has(move |scope, who, role| {
				assert_eq!(who, CONTRACT_ACCOUNT_ID);
				assert!(matches!(scope, PermissionScope::Pool(POOL_ID)));
				assert!(matches!(
					role,
					Role::PoolRole(PoolRole::TrancheInvestor(TRANCHE_ID, NOW_SECS))
				));
				true
			});
			Pools::mock_pool_exists(|_| true);
			Pools::mock_tranche_exists(|_, _| true);
			Tokens::mint_into(
				TRANCHE_CURRENCY,
				&EVM_DOMAIN_ADDRESS.domain().into_account(),
				AMOUNT,
			)
			.unwrap();

			TransferFilter::mock_check(|_| Ok(()));
			Gateway::mock_submit(|sender, destination, msg| {
				assert_eq!(sender, CONTRACT_ACCOUNT_ID);
				assert_eq!(destination, EVM_DOMAIN_ADDRESS.domain());
				assert_eq!(
					msg,
					Message::TransferTrancheTokens {
						pool_id: POOL_ID,
						tranche_id: TRANCHE_ID,
						sender: CONTRACT_ACCOUNT_ID.into(),
						domain: EVM_DOMAIN_ADDRESS.domain().into(),
						receiver: EVM_DOMAIN_ADDRESS.address(),
						amount: AMOUNT
					}
				);
				Ok(())
			});

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::TransferTrancheTokens {
					pool_id: POOL_ID,
					tranche_id: TRANCHE_ID,
					sender: ALICE.into(),
					domain: EVM_DOMAIN_ADDRESS.domain().into(),
					receiver: EVM_DOMAIN_ADDRESS.address(),
					amount: AMOUNT
				}
			));
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_zero_balance() {
			System::externalities().execute_with(|| {
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: ALICE.into(),
							domain: EVM_DOMAIN_ADDRESS.domain().into(),
							receiver: EVM_DOMAIN_ADDRESS.address(),
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Time::mock_now(|| NOW);
				Permissions::mock_has(|_, _, _| false);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: ALICE.into(),
							domain: EVM_DOMAIN_ADDRESS.domain().into(),
							receiver: EVM_DOMAIN_ADDRESS.address(),
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Time::mock_now(|| NOW);
				Permissions::mock_has(|_, _, _| true);
				Pools::mock_pool_exists(|_| false);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: ALICE.into(),
							domain: EVM_DOMAIN_ADDRESS.domain().into(),
							receiver: EVM_DOMAIN_ADDRESS.address(),
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Time::mock_now(|| NOW);
				Permissions::mock_has(|_, _, _| true);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| false);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: ALICE.into(),
							domain: EVM_DOMAIN_ADDRESS.domain().into(),
							receiver: EVM_DOMAIN_ADDRESS.address(),
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Time::mock_now(|| NOW);
				Permissions::mock_has(|_, _, _| true);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);

				assert_noop!(
					LiquidityPools::submit(
						EVM_DOMAIN_ADDRESS,
						Message::TransferTrancheTokens {
							pool_id: POOL_ID,
							tranche_id: TRANCHE_ID,
							sender: ALICE.into(),
							domain: EVM_DOMAIN_ADDRESS.domain().into(),
							receiver: EVM_DOMAIN_ADDRESS.address(),
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

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Pools::mock_pool_exists(|_| true);
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);
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

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				ForeignInvestment::mock_investment(|_, _| Ok(AMOUNT));
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				ForeignInvestment::mock_investment(|_, _| Ok(AMOUNT));
				Pools::mock_pool_exists(|_| true);
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				ForeignInvestment::mock_investment(|_, _| Ok(AMOUNT));
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);
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

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
			DomainAddressToAccountId::mock_convert(|_| ALICE);
			Pools::mock_pool_exists(|_| true);
			Pools::mock_tranche_exists(|_, _| true);
			AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));
			Tokens::mint_into(
				TRANCHE_CURRENCY,
				&EVM_DOMAIN_ADDRESS.domain().into_account(),
				AMOUNT,
			)
			.unwrap();
			ForeignInvestment::mock_increase_foreign_redemption(
				|who, investment_id, amount, foreign_currency| {
					assert_eq!(*who, ALICE);
					assert_eq!(investment_id, INVESTMENT_ID);
					assert_eq!(amount, AMOUNT);
					assert_eq!(foreign_currency, CURRENCY_ID);
					Ok(())
				},
			);

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
		});
	}

	mod erroring_out {
		use super::*;

		#[test]
		fn with_wrong_pool() {
			System::externalities().execute_with(|| {
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Pools::mock_pool_exists(|_| true);
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);
				AssetRegistry::mock_metadata(|_| Some(util::default_metadata()));

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

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
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

			assert_ok!(LiquidityPools::submit(
				EVM_DOMAIN_ADDRESS,
				Message::CancelRedeemOrder {
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				ForeignInvestment::mock_redemption(|_, _| Ok(AMOUNT));
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				ForeignInvestment::mock_redemption(|_, _| Ok(AMOUNT));
				Pools::mock_pool_exists(|_| true);
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
				DomainAccountToDomainAddress::mock_convert(|_| CENTRIFUGE_DOMAIN_ADDRESS);
				DomainAddressToAccountId::mock_convert(|_| ALICE);
				ForeignInvestment::mock_redemption(|_, _| Ok(AMOUNT));
				Pools::mock_pool_exists(|_| true);
				Pools::mock_tranche_exists(|_, _| true);
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
