// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_types::investments::Swap;
use frame_support::{dispatch::fmt::Debug, ensure};
use sp_runtime::{
	traits::{EnsureAdd, EnsureSub},
	DispatchError,
};

use crate::types::{RedeemState, RedeemTransition};

impl<Balance, Currency> RedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
{
	/// Solely apply state machine to transition one `RedeemState` into another
	/// based on the transition, see <https://centrifuge.hackmd.io/IPtRlOrOSrOF9MHjEY48BA?view#Redemption-States>
	///
	/// NOTE: MUST call `apply_redeem_state_transition` on the post state to
	/// actually mutate storage.
	pub fn transition(
		&self,
		transition: RedeemTransition<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match transition {
			RedeemTransition::IncreaseRedeemOrder(amount) => Self::handle_increase(self, amount),
			RedeemTransition::DecreaseRedeemOrder(amount) => Self::handle_decrease(self, amount),
			RedeemTransition::FulfillSwapOrder(swap) => {
				Self::handle_fulfilled_swap_order(self, swap)
			}
			RedeemTransition::CollectRedemption(amount_redeeming, swap) => {
				Self::handle_collect(self, amount_redeeming, swap)
			}
		}
	}

	/// Returns the potentially existing active swap into foreign currency:
	/// * If the state includes `ActiveSwapIntoForeignCurrency`, it returns the
	///   corresponding `Some(swap)`.
	/// * Else, it returns `None`.
	pub(crate) fn get_active_swap(&self) -> Option<Swap<Balance, Currency>> {
		match *self {
			Self::ActiveSwapIntoForeignCurrency { swap }
			| Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. }
			| Self::RedeemingAndActiveSwapIntoForeignCurrency { swap, .. }
			| Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				swap, ..
			} => Some(swap),
			_ => None,
		}
	}

	/// Returns the redeeming amount if existent. Else returns zero.
	pub(crate) fn get_redeeming_amount(&self) -> Balance {
		match *self {
			Self::Redeeming { redeem_amount }
			| Self::RedeemingAndActiveSwapIntoForeignCurrency { redeem_amount, .. }
			| Self::RedeemingAndSwapIntoForeignDone { redeem_amount, .. }
			| Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				redeem_amount,
				..
			} => redeem_amount,
			_ => Balance::zero(),
		}
	}

	/// Either adds a non existing redeeming amount to the state or overwrites
	/// it.
	/// * If the value is not zero and the state involves `Redeeming`: Sets the
	///   amount.
	/// * Else if the value is not zero and the state does not involve
	///   `Redeeming`: Adds `Redeeming` to the state with the corresponding
	///   amount.
	/// * If the value is zero and the state includes `Redeeming`: Removes
	///   `Redeeming` from the state.
	/// * Else throws.
	fn set_redeem_amount(&self, amount: Balance) -> Result<Self, DispatchError> {
		if amount.is_zero() {
			return Self::remove_redeem_amount(self);
		}
		match *self {
			Self::NoState | Self::Redeeming { .. } => Ok(Self::Redeeming {
				redeem_amount: amount,
			}),
			Self::RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } => {
				Ok(Self::RedeemingAndActiveSwapIntoForeignCurrency {
					redeem_amount: amount,
					swap,
				})
			}
			Self::RedeemingAndSwapIntoForeignDone { done_swap, .. } => {
				Ok(Self::RedeemingAndSwapIntoForeignDone {
					redeem_amount: amount,
					done_swap,
				})
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				swap,
				done_amount,
				..
			} => Ok(
				Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					redeem_amount: amount,
					swap,
					done_amount,
				},
			),
			Self::ActiveSwapIntoForeignCurrency { swap } => {
				Ok(Self::RedeemingAndActiveSwapIntoForeignCurrency {
					swap,
					redeem_amount: amount,
				})
			}
			Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => Ok(
				Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					swap,
					done_amount,
					redeem_amount: amount,
				},
			),
			Self::SwapIntoForeignDone { done_swap } => Ok(Self::RedeemingAndSwapIntoForeignDone {
				done_swap,
				redeem_amount: amount,
			}),
		}
	}

	/// Removes `Redeeming` from the state.
	fn remove_redeem_amount(&self) -> Result<Self, DispatchError> {
		match *self {
			Self::Redeeming { .. } => Ok(Self::NoState),
			Self::RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } => {
				Ok(Self::ActiveSwapIntoForeignCurrency { swap })
			}
			Self::RedeemingAndSwapIntoForeignDone { done_swap, .. } => {
				Ok(Self::SwapIntoForeignDone { done_swap })
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				swap,
				done_amount,
				..
			} => Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount }),
			// Throw for states without `Redeeming`
			_ => Err(DispatchError::Other(
				"Cannot remove redeeming amount of redeem state which does not include `Redeeming`",
			)),
		}
	}

	/// Reduce the amount of an active swap (into foreign currency) by the
	/// provided value:
	/// * Throws if there is no active swap, i.e. the state does not include
	///   `ActiveSwapIntoForeignCurrency` or if the reducible amount exceeds the
	///   swap amount
	/// * If the provided value equals the swap amount, the state is
	///   transitioned into `*AndSwapIntoForeignDone`.
	/// * Else, it is transitioned into
	///   `*ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone`.
	pub(crate) fn fulfill_active_swap_amount(
		&self,
		amount: Balance,
	) -> Result<Self, DispatchError> {
		match self {
			Self::ActiveSwapIntoForeignCurrency { swap } => {
				if amount == swap.amount {
					Ok(Self::SwapIntoForeignDone { done_swap: *swap })
				} else {
					Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: Swap {
							amount: swap.amount.ensure_sub(amount)?,
							..*swap
						},
						done_amount: amount,
					})
				}
			}
			Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(Self::SwapIntoForeignDone {
						done_swap: Swap {
							amount: done_amount,
							..*swap
						},
					})
				} else {
					Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: Swap {
							amount: swap.amount.ensure_sub(amount)?,
							..*swap
						},
						done_amount,
					})
				}
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrency {
				redeem_amount,
				swap,
			} => {
				if amount == swap.amount {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						done_swap: Swap { amount, ..*swap },
						redeem_amount: *redeem_amount,
					})
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount: amount,
							redeem_amount: *redeem_amount,
						},
					)
				}
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				redeem_amount,
				swap,
				done_amount,
			} => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						done_swap: Swap {
							amount: done_amount,
							..*swap
						},
						redeem_amount: *redeem_amount,
					})
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount,
							redeem_amount: *redeem_amount,
						},
					)
				}
			}
			_ => Err(DispatchError::Other(
				"Invalid redeem state when fulfilling active swap amount",
			)),
		}
	}

	/// Transition all states which include `ActiveSwapIntoForeignCurrency`.
	///
	/// The resulting transitioned state either includes `*SwapIntoForeignDone`
	/// or `*ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone`.
	///
	/// Also supports non-foreign swaps, i.e. those with matching in and out
	/// currency.
	///
	/// Throws if the fulfilled swap direction is not into foreign currency or
	/// if the amount exceeds the states active swap amount.
	fn transition_fulfilled_swap_order(
		&self,
		fulfilled_swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		ensure!(
			self.get_active_swap()
				.map(|swap| {
					swap.amount >= fulfilled_swap.amount
						&& swap.currency_in == fulfilled_swap.currency_in
						&& swap.currency_out == fulfilled_swap.currency_out
				})
				.unwrap_or(true),
			DispatchError::Other("Invalid redeem state when transitioning fulfilled swap order")
		);

		let Swap { amount, .. } = fulfilled_swap;

		// Edge case: if currency_in matches currency_out, we can immediately fulfill
		// the swap
		match *self {
			Self::ActiveSwapIntoForeignCurrency { swap } => {
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: Swap {
							amount: swap.amount - amount,
							..swap
						},
						done_amount: amount,
					})
				} else {
					Ok(Self::SwapIntoForeignDone { done_swap: swap })
				}
			}
			Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: Swap {
							amount: swap.amount - amount,
							..swap
						},
						done_amount,
					})
				} else {
					Ok(Self::SwapIntoForeignDone {
						done_swap: Swap {
							amount: done_amount,
							..swap
						},
					})
				}
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrency {
				redeem_amount,
				swap,
			} => {
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount - amount,
								..swap
							},
							done_amount: amount,
							redeem_amount,
						},
					)
				} else {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						done_swap: swap,
						redeem_amount,
					})
				}
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				redeem_amount,
				swap,
				done_amount,
			} => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount - amount,
								..swap
							},
							done_amount,
							redeem_amount,
						},
					)
				} else {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						done_swap: Swap {
							amount: done_amount,
							..swap
						},
						redeem_amount,
					})
				}
			}
			_ => Err(DispatchError::Other(
				"Invalid redeem state when transitioning fulfilled swap order",
			)),
		}
	}

	/// Either update or remove the redeeming amount and add
	/// `SwapIntoForeignDone` for the provided collected swap.
	fn transition_collect_non_foreign(
		&self,
		amount_redeeming: Balance,
		collected_swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match *self {
			Self::Redeeming { .. } => {
				if amount_redeeming.is_zero() {
					Ok(Self::SwapIntoForeignDone {
						done_swap: collected_swap,
					})
				} else {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						redeem_amount: amount_redeeming,
						done_swap: collected_swap,
					})
				}
			}
			Self::RedeemingAndSwapIntoForeignDone { done_swap, .. } => {
				let swap = Swap {
					amount: done_swap.amount.ensure_add(collected_swap.amount)?,
					..collected_swap
				};

				if amount_redeeming.is_zero() {
					Ok(Self::SwapIntoForeignDone { done_swap: swap })
				} else {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						redeem_amount: amount_redeeming,
						done_swap: swap,
					})
				}
			}
			_ => Err(DispatchError::Other(
				"Invalid pre redeem state when transitioning non-foreign collect",
			)),
		}
	}

	/// Apply the transition of the state after collecting a redemption:
	/// * Either remove or update the redeeming amount
	/// * Either add or update an active swap into foreign currency (or note a
	///   fulfilled swap if the in and out currencies are the same).
	///
	/// Throws if any of the following holds true
	/// * The current state includes an active/done swap and in and out
	///   currencies do not match the provided ones
	/// * The collected amount is zero but there is a mismatch between the
	///   redeeming amounts (which can only be possible if something was
	///   collected)
	/// * The state does not include `Redeeming`
	fn transition_collect(
		&self,
		amount_redeeming: Balance,
		collected_swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		let redeeming_amount = self.get_redeeming_amount();

		ensure!(
			self.get_active_swap()
				.map(|swap| (swap.currency_in, swap.currency_out)
					== (collected_swap.currency_in, collected_swap.currency_out))
				.unwrap_or(true),
			DispatchError::Other("Invalid swap currencies when transitioning collect redemption")
		);

		// Nothing changed in the executed epoch
		if collected_swap.amount.is_zero() {
			if redeeming_amount == amount_redeeming {
				return Ok(*self);
			} else {
				return Err(DispatchError::Other(
					"Corruption: Redeeming amount changed but nothing was collected",
				));
			}
		}

		// Take shortcut for same currencies
		if collected_swap.currency_in == collected_swap.currency_out {
			return Self::transition_collect_non_foreign(self, amount_redeeming, collected_swap);
		}

		// Either remove or update redeeming amount and add/update swap into foreign
		// currency
		match *self {
			Self::Redeeming { .. } => {
				if amount_redeeming.is_zero() {
					Ok(Self::ActiveSwapIntoForeignCurrency {
						swap: collected_swap,
					})
				} else {
					Ok(Self::RedeemingAndActiveSwapIntoForeignCurrency {
						redeem_amount: amount_redeeming,
						swap: collected_swap,
					})
				}
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } => {
				let new_swap = Swap {
					amount: swap.amount.ensure_add(collected_swap.amount)?,
					..collected_swap
				};
				if amount_redeeming.is_zero() {
					Ok(Self::ActiveSwapIntoForeignCurrency { swap: new_swap })
				} else {
					Ok(Self::RedeemingAndActiveSwapIntoForeignCurrency {
						redeem_amount: amount_redeeming,
						swap: new_swap,
					})
				}
			}
			Self::RedeemingAndSwapIntoForeignDone { done_swap, .. } => {
				if amount_redeeming.is_zero() {
					Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: collected_swap,
						done_amount: done_swap.amount,
					})
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							redeem_amount: amount_redeeming,
							swap: collected_swap,
							done_amount: done_swap.amount,
						},
					)
				}
			}
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				swap,
				done_amount,
				..
			} => {
				let new_swap = Swap {
					amount: swap.amount.ensure_add(collected_swap.amount)?,
					..collected_swap
				};
				if amount_redeeming.is_zero() {
					Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: new_swap,
						done_amount,
					})
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							redeem_amount: amount_redeeming,
							swap: new_swap,
							done_amount,
						},
					)
				}
			}
			_ => Err(DispatchError::Other(
				"Invalid pre redeem state when transitioning foreign collect",
			)),
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> RedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
{
	/// Increments the unprocessed redeeming amount or adds `Redeeming*` to the
	/// state with the provided amount.
	fn handle_increase(&self, amount: Balance) -> Result<Self, DispatchError> {
		Self::set_redeem_amount(self, Self::get_redeeming_amount(self).ensure_add(amount)?)
	}

	/// Decrement the unprocessed redeeming amount. I.e., if the state includes
	/// `Redeeming*`, decreases the redeeming amount.
	fn handle_decrease(&self, amount: Balance) -> Result<Self, DispatchError> {
		Self::set_redeem_amount(self, Self::get_redeeming_amount(self).ensure_sub(amount)?)
	}

	/// Update the state if it includes `ActiveSwapIntoForeignCurrency`.
	fn handle_fulfilled_swap_order(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match self {
			Self::NoState => Err(DispatchError::Other(
				"Invalid redeem state when transitioning a fulfilled order",
			)),
			state => state.transition_fulfilled_swap_order(swap),
		}
	}

	/// Update the state if it includes `Redeeming`.
	fn handle_collect(
		&self,
		amount_redeeming: Balance,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match self {
			Self::NoState => Err(DispatchError::Other(
				"Invalid redeem state when transitioning collect",
			)),
			state => state.transition_collect(amount_redeeming, swap),
		}
	}
}

#[cfg(test)]
mod tests {
	use frame_support::{assert_err, assert_ok};
	use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};

	use super::*;

	#[derive(Clone, Copy, PartialEq, Debug)]
	enum CurrencyId {
		Foreign,
		Pool,
	}

	type RedeemState = super::RedeemState<u128, CurrencyId>;
	type RedeemTransition = super::RedeemTransition<u128, CurrencyId>;

	impl RedeemState {
		fn get_done_amount(&self) -> u128 {
			match *self {
				Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					done_amount, ..
				} => done_amount,
				Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					done_amount,
					..
				} => done_amount,
				_ => 0,
			}
		}

		fn get_swap_amount(&self) -> u128 {
			match *self {
				Self::ActiveSwapIntoForeignCurrency { swap } => swap.amount,
				Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. } => {
					swap.amount
				}
				Self::RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } => swap.amount,
				Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					swap,
					..
				} => swap.amount,
				_ => 0,
			}
		}

		fn total(&self) -> u128 {
			self.get_redeeming_amount() + self.get_done_amount() + self.get_swap_amount()
		}
	}

	struct Checker {
		old_state: RedeemState,
	}

	impl Checker {
		fn check_delta_invariant(&self, transition: &RedeemTransition, new_state: &RedeemState) {
			dbg!(
				transition,
				self.old_state.total(),
				new_state.total(),
				new_state
			);
			match transition {
				RedeemTransition::IncreaseRedeemOrder(amount) => {
					let diff = new_state.total() - self.old_state.total();
					assert_eq!(diff, *amount);
				}
				RedeemTransition::DecreaseRedeemOrder(amount) => {
					let diff = self.old_state.total() - new_state.total();
					assert_eq!(diff, *amount);
				}
				RedeemTransition::FulfillSwapOrder(swap) => (),
				RedeemTransition::CollectRedemption(value, swap) => (),
			};
		}
	}

	#[test]
	fn fuzzer() {
		let transitions = [
			RedeemTransition::IncreaseRedeemOrder(120),
			RedeemTransition::IncreaseRedeemOrder(60),
			RedeemTransition::DecreaseRedeemOrder(120),
			RedeemTransition::DecreaseRedeemOrder(60),
			//RedeemTransition::FulfillSwapOrder(pool_swap_big),
			//RedeemTransition::FulfillSwapOrder(pool_swap_small),
			//RedeemTransition::FulfillSwapOrder(foreign_swap_big),
			//RedeemTransition::FulfillSwapOrder(foreign_swap_small),
			//RedeemTransition::CollectInvestment(60),
			//RedeemTransition::CollectInvestment(120),
		];

		let mut rng = StdRng::seed_from_u64(42); // Determinism for reproduction

		for _ in 0..10000 {
			let use_case = transitions.clone().into_iter().choose_multiple(&mut rng, 8);

			println!("Testing use case: {:#?}", use_case);

			let mut state = RedeemState::NoState;
			let mut checker = Checker {
				old_state: state.clone(),
			};

			for transition in use_case {
				state = state
					.transition(transition.clone())
					.unwrap_or(state.clone());

				checker.check_delta_invariant(&transition, &state);
				checker.old_state = state.clone();
			}
		}
	}
}
