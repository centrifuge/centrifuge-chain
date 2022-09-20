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

/*
type account_id = u64;
type investment_id = u32;
type currency_id = u32;
type balance = u128;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct GenesisConfig {
	infos: Vec<(investment_id, InvestmentInfo)>,
}

impl frame_support::traits::GenesisBuild<()> for GenesisConfig {
	fn build(&self) {
		STATE.with(|s| {
			let mut state = s.borrow_mut();

			for (id, info) in &self.infos {
				state.add(id.clone(), info.clone())
			}
		})
	}
}

pub struct name<Tokens>(sp_std::marker::PhantomData<Tokens>);

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct InvestmentInfo {
	owner: account_id,
	id: investment_id,
	payment_currency: currency_id,
}

impl<Tokens> InvestmentAccountant<account_id> for name<Tokens>
where
	Tokens: frame_support::traits::tokens::fungibles::Mutate<account_id>
		+ frame_support::traits::tokens::fungibles::Transfer<account_id>
		+ frame_support::traits::tokens::fungibles::Inspect<account_id, Balance = balance>,
	<Tokens as frame_support::traits::tokens::fungibles::Inspect<account_id>>::AssetId:
		From<investment_id>,
{
	type Amount = balance;
	type Error = frame_support::dispatch::DispatchError;
	type InvestmentId = investment_id;
	type InvestmentInfo = InvestmentInfo;

	fn info(id: Self::InvestmentId) -> Result<Self::InvestmentInfo, Self::Error> {
		STATE.with(|s| s.borrow().info(&id))
	}

	fn transfer(
		id: Self::InvestmentId,
		source: &account_id,
		dest: &account_id,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let _ = STATE.with(|s| s.borrow().info(&id))?;

		Tokens::transfer(id.into(), source, dest, amount, false).map(|_| ())
	}

	fn deposit(
		buyer: &account_id,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let _ = STATE.with(|s| s.borrow().info(&id))?;

		Tokens::mint_into(id.into(), buyer, amount)
	}

	fn withdraw(
		seller: &account_id,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let _ = STATE.with(|s| s.borrow().info(&id))?;

		Tokens::burn_from(id.into(), seller, amount).map(|_| ())
	}
}

impl InvestmentProperties<account_id> for InvestmentInfo {
	type Currency = currency_id;
	type Id = investment_id;

	fn owner(&self) -> account_id {
		self.owner
	}

	fn id(&self) -> Self::Id {
		self.id
	}

	fn payment_currency(&self) -> Self::Currency {
		self.payment_currency
	}
}

pub struct AccountantState {
	infos: Vec<(investment_id, InvestmentInfo)>,
}

impl AccountantState {
	pub fn new() -> Self {
		Self {
			infos: Vec::default(),
		}
	}

	pub fn info(
		&self,
		investment_id: &investment_id,
	) -> Result<InvestmentInfo, frame_support::dispatch::DispatchError> {
		for (curr_id, info) in &self.infos {
			if curr_id == investment_id {
				return Ok(info.clone());
			}
		}

		Err(frame_support::dispatch::DispatchError::Other(
			"No info for investment_id available",
		))
	}

	pub fn add(&mut self, investment_id: investment_id, info: InvestmentInfo) {
		self.infos.push((investment_id, info))
	}
}

thread_local! {
	pub static STATE: sp_std::cell::RefCell<
		AccountantState,
		> = sp_std::cell::RefCell::new(AccountantState::new());
}
*/
/// Exposes a struct $name that implements the `trait Accountant`. The struct expects one generic
/// parameter that implements the fungibles traits `Inspect`, `Mutate` and `Transfer`. Furthermore,
/// there exists a struct `GenesisConfig` that implements `trait GenesisBuild` that can be used
/// like any other `GenesisConfig` to initialize state in the `TestExternalities`.
///
/// Also exports a `struct InvestmentInfo` to be used in the `GenesisConfig`
///
/// * E.g.: `MockAccountant<Tokens: frame_support::traits::tokens::fungibles::{Inspect, Mutate, Transfer}>`
///
/// # Example macro usage:
/// ```rust
/// use cfg_traits::impl_mock_accountant;
/// use cfg_primitives::{PoolId, TrancheId, Balance};
/// use cfg_types::CurrencyId;
/// use frame_support::traits::fungibles::{Inspect, Mutate, Transfer};
/// use frame_support::traits::GenesisBuild;
///
/// /// The used account id for this mock
/// type AccountId = u64;
///
/// enum InvestmentId {
///     Tranches(PoolId, TrancheId),
/// }
///
/// impl Into<CurrencyId> for InvestmentId {
/// 	fn into(self) -> CurrencyId {
///    		CurrencyId::Tranche(self.0, self.1)
///     }
/// }
///
///
/// impl_mock_accountant!(
/// 	MockAccountant,
///     AccountId,
///     InvestmentId,
///     CurrencyId,
///     Balance,
/// );
///
/// // Using the `GenesisConfig`
/// let storage = GenesisBuild::build_storage(accountant_mock::GenesisConfig {
///			infos: vec![
/// 			(
/// 				InvestmentId::Tranche(0, [0;16]),
/// 				accountant_mock::InvestmentInfo {
/// 					owner: 1,
/// 					id: InvestmentId::Tranche(0, [0;16]),
/// 					payment_currency:CurrencyId::AUSD
/// 				}
/// 			)
/// 		]
/// }).expect("Must not fail");
/// ```
#[macro_export]
macro_rules! impl_mock_accountant {
	($name:ident, $account_id:ty, $investment_id:ty, $currency_id:ty, $balance:ty) => {
		pub use accountant_mock::$name;

		mod accountant_mock {
			use std::borrow::{Borrow as _, BorrowMut as _};

			use __private::STATE as __private_STATE;

			use super::*;

			#[derive(Default, serde::Serialize, serde::Deserialize)]
			pub struct GenesisConfig {
				pub infos: Vec<($investment_id, InvestmentInfo)>,
			}

			impl frame_support::traits::GenesisBuild<()> for GenesisConfig {
				fn build(&self) {
					__private_STATE.with(|s| {
						let mut state = s.borrow_mut();

						for (id, info) in &self.infos {
							state.add(id.clone(), info.clone())
						}
					})
				}
			}

			pub struct $name<Tokens>(sp_std::marker::PhantomData<Tokens>);

			#[derive(Clone, serde::Serialize, serde::Deserialize)]
			pub struct InvestmentInfo {
				pub owner: $account_id,
				pub id: $investment_id,
				pub payment_currency: $currency_id,
			}

			impl<Tokens> cfg_traits::InvestmentAccountant<$account_id> for $name<Tokens>
			where
				Tokens: frame_support::traits::tokens::fungibles::Mutate<$account_id>
					+ frame_support::traits::tokens::fungibles::Transfer<$account_id>
					+ frame_support::traits::tokens::fungibles::Inspect<
						$account_id,
						Balance = $balance,
						AssetId = $currency_id,
					>,
				$investment_id:
					Into<
						<Tokens as frame_support::traits::tokens::fungibles::Inspect<
							$account_id,
						>>::AssetId,
					>,
			{
				type Amount = $balance;
				type Error = frame_support::dispatch::DispatchError;
				type InvestmentId = $investment_id;
				type InvestmentInfo = InvestmentInfo;

				fn info(id: Self::InvestmentId) -> Result<Self::InvestmentInfo, Self::Error> {
					__private_STATE.with(|s| s.borrow().info(&id))
				}

				fn balance(id: Self::InvestmentId, who: &$account_id) -> Self::Amount {
					Tokens::balance(id.into(), who)
				}

				fn transfer(
					id: Self::InvestmentId,
					source: &$account_id,
					dest: &$account_id,
					amount: Self::Amount,
				) -> Result<(), Self::Error> {
					let _ = __private_STATE.with(|s| s.borrow().info(&id))?;

					Tokens::transfer(id.into(), source, dest, amount, false).map(|_| ())
				}

				fn deposit(
					buyer: &$account_id,
					id: Self::InvestmentId,
					amount: Self::Amount,
				) -> Result<(), Self::Error> {
					let _ = __private_STATE.with(|s| s.borrow().info(&id))?;

					Tokens::mint_into(id.into(), buyer, amount)
				}

				fn withdraw(
					seller: &$account_id,
					id: Self::InvestmentId,
					amount: Self::Amount,
				) -> Result<(), Self::Error> {
					let _ = __private_STATE.with(|s| s.borrow().info(&id))?;

					Tokens::burn_from(id.into(), seller, amount).map(|_| ())
				}
			}

			impl cfg_traits::InvestmentProperties<$account_id> for InvestmentInfo {
				type Currency = $currency_id;
				type Id = $investment_id;

				fn owner(&self) -> $account_id {
					self.owner
				}

				fn id(&self) -> Self::Id {
					self.id
				}

				fn payment_currency(&self) -> Self::Currency {
					self.payment_currency
				}
			}

			mod __private {
				use super::*;

				pub struct AccountantState {
					infos: Vec<($investment_id, InvestmentInfo)>,
				}

				impl AccountantState {
					pub fn new() -> Self {
						Self {
							infos: Vec::default(),
						}
					}

					pub fn info(
						&self,
						investment_id: &$investment_id,
					) -> Result<InvestmentInfo, frame_support::dispatch::DispatchError> {
						for (curr_id, info) in &self.infos {
							if curr_id == investment_id {
								return Ok(info.clone());
							}
						}

						Err(frame_support::dispatch::DispatchError::Other(
							"No info for investment_id available",
						))
					}

					pub fn add(&mut self, investment_id: $investment_id, info: InvestmentInfo) {
						// NOTE: We deliberately update the info here as add() is only called
						//       upon GenesisConfig.build(). We assume, if we are running in the
						//       same thread this means a new initialization is wanted.
						for (curr_id, curr_info) in &mut self.infos {
							if *curr_id == investment_id {
								*curr_info = info;
								return;
							}
						}

						self.infos.push((investment_id, info))
					}
				}

				thread_local! {
					pub static STATE: sp_std::cell::RefCell<
						AccountantState,
						> = sp_std::cell::RefCell::new(AccountantState::new());
				}
			}
		}
	};
}

pub use impl_mock_accountant;
