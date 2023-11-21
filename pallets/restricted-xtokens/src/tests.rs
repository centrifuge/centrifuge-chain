// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg(test)]

use cumulus_primitives_core::ParaId;
use frame_support::{assert_noop, assert_ok, traits::Currency};
use mock::*;
use orml_traits::MultiCurrency;
use sp_runtime::{traits::AccountIdConversion, AccountId32};
use xcm_simulator::TestExt;

use super::*;

fn para_a_account() -> AccountId32 {
	ParaId::from(1).into_account_truncating()
}

// Not used in any unit tests, but it's super helpful for debugging. Let's
// keep it here.
#[allow(dead_code)]
fn print_events<Runtime: frame_system::Config>(name: &'static str) {
	println!("------ {:?} events -------", name);
	frame_system::Pallet::<Runtime>::events()
		.iter()
		.for_each(|r| println!("> {:?}", r.event));
}

mod para_a_to_relay {
	use super::*;

	#[test]
	fn restrict_normal_transfer_relay_currency() {
		TestNet::reset();

		Relay::execute_with(|| {
			let _ = RelayBalances::deposit_creating(&para_a_account(), 1_000);
		});

		ParaA::execute_with(|| {
			assert_noop!(
				ParaXTokens::transfer(
					Some(RESTRICTED_SENDER).into(),
					CurrencyId::R,
					500,
					Box::new(
						MultiLocation::new(
							1,
							X1(Junction::AccountId32 {
								network: None,
								id: BOB.into(),
							})
						)
						.into()
					),
					WeightLimit::Unlimited
				),
				Error::<para::Runtime>::RestrictionTriggered
			);

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_SENDER),
				1000
			);
		});

		Relay::execute_with(|| {
			assert_eq!(RelayBalances::free_balance(&para_a_account()), 1000);
			assert_eq!(RelayBalances::free_balance(&BOB), 0);
		});
	}

	#[test]
	fn allow_normal_transfer_relay_currency() {
		TestNet::reset();

		Relay::execute_with(|| {
			let _ = RelayBalances::deposit_creating(&para_a_account(), 1_000);
		});

		ParaA::execute_with(|| {
			assert_ok!(ParaXTokens::transfer(
				Some(RESTRICTED_SENDER).into(),
				CurrencyId::R,
				500,
				Box::new(
					MultiLocation::new(
						1,
						X1(Junction::AccountId32 {
							network: None,
							id: RESTRICTED_RECEIVER.into(),
						})
					)
					.into()
				),
				WeightLimit::Unlimited
			));
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_SENDER),
				500
			);
		});

		Relay::execute_with(|| {
			assert_eq!(RelayBalances::free_balance(&para_a_account()), 500);
			assert_eq!(RelayBalances::free_balance(&RESTRICTED_RECEIVER), 460);
		});
	}
}

mod para_a_to_para_a {
	use super::*;

	#[test]
	fn restrict_normal_transfer_relay_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_noop!(
				ParaXTokens::transfer(
					Some(RESTRICTED_SENDER).into(),
					CurrencyId::R,
					500,
					Box::new(
						MultiLocation::new(
							0,
							X1(Junction::AccountId32 {
								network: None,
								id: BOB.into(),
							})
						)
						.into()
					),
					WeightLimit::Unlimited
				),
				Error::<para::Runtime>::RestrictionTriggered
			);

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_SENDER),
				1000
			);
		});
	}

	#[test]
	fn restrict_normal_transfer_a1_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_noop!(
				ParaXTokens::transfer(
					Some(RESTRICTED_SENDER).into(),
					CurrencyId::A1,
					500,
					Box::new(
						MultiLocation::new(
							0,
							X1(Junction::AccountId32 {
								network: None,
								id: BOB.into(),
							})
						)
						.into()
					),
					WeightLimit::Unlimited
				),
				Error::<para::Runtime>::RestrictionTriggered
			);

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_SENDER),
				1000
			);
		});
	}

	#[test]
	fn restrict_normal_transfer_b1_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_noop!(
				ParaXTokens::transfer(
					Some(RESTRICTED_SENDER).into(),
					CurrencyId::B1,
					500,
					Box::new(
						MultiLocation::new(
							0,
							X1(Junction::AccountId32 {
								network: None,
								id: BOB.into(),
							})
						)
						.into()
					),
					WeightLimit::Unlimited
				),
				Error::<para::Runtime>::RestrictionTriggered
			);

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::B1, &RESTRICTED_SENDER),
				1000
			);
		});
	}
}

mod para_a_to_para_b {
	use super::*;

	#[test]
	fn restrict_normal_transfer_relay_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_noop!(
				ParaXTokens::transfer(
					Some(RESTRICTED_SENDER).into(),
					CurrencyId::R,
					500,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Junction::Parachain(2),
								Junction::AccountId32 {
									network: None,
									id: BOB.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Unlimited
				),
				Error::<para::Runtime>::RestrictionTriggered
			);

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_SENDER),
				1000
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_RECEIVER),
				0
			);
		})
	}

	#[test]
	fn allow_normal_transfer_relay_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_ok!(ParaXTokens::transfer(
				Some(RESTRICTED_SENDER).into(),
				CurrencyId::R,
				500,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Junction::Parachain(2),
							Junction::AccountId32 {
								network: None,
								id: RESTRICTED_RECEIVER.into(),
							}
						)
					)
					.into()
				),
				WeightLimit::Unlimited
			));

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_SENDER),
				500
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_RECEIVER),
				0
			);
		})
	}

	#[test]
	fn restrict_normal_transfer_a1_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_noop!(
				ParaXTokens::transfer(
					Some(RESTRICTED_SENDER).into(),
					CurrencyId::A1,
					500,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Junction::Parachain(2),
								Junction::AccountId32 {
									network: None,
									id: BOB.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Unlimited
				),
				Error::<para::Runtime>::RestrictionTriggered
			);

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_SENDER),
				1000
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_RECEIVER),
				0
			);
		})
	}

	#[test]
	fn allow_normal_transfer_a1_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_ok!(ParaXTokens::transfer(
				Some(RESTRICTED_SENDER).into(),
				CurrencyId::A1,
				500,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Junction::Parachain(2),
							Junction::AccountId32 {
								network: None,
								id: RESTRICTED_RECEIVER.into(),
							}
						)
					)
					.into()
				),
				WeightLimit::Unlimited
			));

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::A1, &RESTRICTED_SENDER),
				500
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::A1, &RESTRICTED_RECEIVER),
				460
			);
		})
	}

	#[test]
	fn restrict_normal_transfer_b1_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_noop!(
				ParaXTokens::transfer(
					Some(RESTRICTED_SENDER).into(),
					CurrencyId::B1,
					500,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Junction::Parachain(2),
								Junction::AccountId32 {
									network: None,
									id: BOB.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Unlimited
				),
				Error::<para::Runtime>::RestrictionTriggered
			);

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::B1, &RESTRICTED_SENDER),
				1000
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::R, &RESTRICTED_RECEIVER),
				0
			);
		})
	}

	#[test]
	fn allow_normal_transfer_b1_currency() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_ok!(ParaXTokens::transfer(
				Some(RESTRICTED_SENDER).into(),
				CurrencyId::B1,
				500,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Junction::Parachain(2),
							Junction::AccountId32 {
								network: None,
								id: RESTRICTED_RECEIVER.into(),
							}
						)
					)
					.into()
				),
				WeightLimit::Unlimited
			));

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::B1, &RESTRICTED_SENDER),
				500
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::B1, &RESTRICTED_RECEIVER),
				0
			);
		})
	}

	#[test]
	fn a_and_b_are_not_restricted() {
		TestNet::reset();

		ParaA::execute_with(|| {
			assert_ok!(ParaXTokens::transfer(
				Some(RESTRICTED_SENDER).into(),
				CurrencyId::A,
				500,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Junction::Parachain(2),
							Junction::AccountId32 {
								network: None,
								id: RESTRICTED_RECEIVER.into(),
							}
						)
					)
					.into()
				),
				WeightLimit::Unlimited
			));

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::A, &RESTRICTED_SENDER),
				500
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::A, &RESTRICTED_RECEIVER),
				460
			);
		});

		ParaA::execute_with(|| {
			assert_ok!(ParaXTokens::transfer(
				Some(RESTRICTED_SENDER).into(),
				CurrencyId::B,
				500,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Junction::Parachain(2),
							Junction::AccountId32 {
								network: None,
								id: RESTRICTED_RECEIVER.into(),
							}
						)
					)
					.into()
				),
				WeightLimit::Unlimited
			));

			assert_eq!(
				ParaTokens::free_balance(CurrencyId::B, &RESTRICTED_SENDER),
				500
			);
		});

		ParaB::execute_with(|| {
			assert_eq!(
				ParaTokens::free_balance(CurrencyId::B, &RESTRICTED_RECEIVER),
				0
			);
		})
	}
}
