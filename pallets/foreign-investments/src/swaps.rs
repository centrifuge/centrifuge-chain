//! Abstracts the swapping logic

use cfg_traits::{IdentityCurrencyConversion, TokenSwaps};
use frame_support::pallet_prelude::*;
use sp_runtime::traits::{EnsureAdd, EnsureSub, One, Zero};
use sp_std::cmp::Ordering;

use crate::{
	pallet::{Config, Error},
	Action, ForeignIdToSwapId, SwapIdToForeignId, SwapOf,
};

/// Internal type used as result of `Pallet::apply_swap()`
/// Amounts are donominated referenced by the `new_swap` paramenter given to
/// `apply_swap()`
#[derive(Debug, PartialEq)]
pub struct SwapStatus<T: Config> {
	/// The amount (in) already swapped and available to use.
	pub swapped: T::Balance,

	/// The amount (in) pending to be swapped
	pub pending: T::Balance,

	/// The swap id for a possible reminder swap order after `apply_swap()`
	pub swap_id: Option<T::SwapId>,
}

/// Type that has methods related to swap actions
pub struct Swaps<T>(PhantomData<T>);
impl<T: Config> Swaps<T> {
	/// Inserts, updates or removes a swap id associated to a foreign
	/// action.
	pub fn update_swap_id(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		action: Action,
		new_swap_id: Option<T::SwapId>,
	) -> DispatchResult {
		let previous_swap_id = ForeignIdToSwapId::<T>::get((who, investment_id, action));

		if previous_swap_id != new_swap_id {
			if let Some(new_id) = new_swap_id {
				SwapIdToForeignId::<T>::insert(new_id, (who.clone(), investment_id, action));
				ForeignIdToSwapId::<T>::insert((who.clone(), investment_id, action), new_id);
			}

			if let Some(old_id) = previous_swap_id {
				SwapIdToForeignId::<T>::remove(old_id);
				ForeignIdToSwapId::<T>::remove((who.clone(), investment_id, action));
			}
		}

		Ok(())
	}

	pub fn pending_swap_amount(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		currency_in: T::CurrencyId,
		action: Action,
	) -> T::Balance {
		ForeignIdToSwapId::<T>::get((who, investment_id, action))
			.map(|swap_id| T::TokenSwaps::get_order_details(swap_id))
			.flatten()
			.filter(|swap| swap.currency_in == currency_in)
			.map(|swap| swap.amount_in)
			.unwrap_or(T::Balance::default())
	}

	/// A wrap over `apply_swap_over_swap()` that makes the swap from an
	/// investment PoV
	pub fn apply_swap(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		action: Action,
		new_swap: SwapOf<T>,
	) -> Result<SwapStatus<T>, DispatchError> {
		let swap_id = ForeignIdToSwapId::<T>::get((who, investment_id, action));

		let status = Swaps::<T>::apply_swap_over_swap(who, new_swap.clone(), swap_id)?;

		Swaps::<T>::update_swap_id(who, investment_id, action, status.swap_id)?;

		Ok(status)
	}

	/// Apply a swap over a current possible swap state.
	/// - If there was no previous swap, it adds it.
	/// - If there was a swap in the same direction, it increments it.
	/// - If there was a swap in the opposite direction:
	///   - If the amount is smaller, it decrements it.
	///   - If the amount is the same, it removes the inverse swap.
	///   - If the amount is greater, it removes the inverse swap and create
	///     another with the excess
	///
	/// The returned status contains the swapped amounts after this call and
	/// the pending amounts to be swapped of both swap directions.
	pub fn apply_swap_over_swap(
		who: &T::AccountId,
		new_swap: SwapOf<T>,
		over_swap_id: Option<T::SwapId>,
	) -> Result<SwapStatus<T>, DispatchError> {
		match over_swap_id {
			None => {
				let swap_id = T::TokenSwaps::place_order(
					who.clone(),
					new_swap.currency_in,
					new_swap.currency_out,
					new_swap.amount_in,
					T::BalanceRatio::one(),
				)?;

				Ok(SwapStatus {
					swapped: T::Balance::zero(),
					pending: new_swap.amount_in,
					swap_id: Some(swap_id),
				})
			}
			Some(swap_id) => {
				let swap = T::TokenSwaps::get_order_details(swap_id)
					.ok_or(Error::<T>::SwapOrderNotFound)?;

				if swap.is_same_direction(&new_swap)? {
					let amount_to_swap = swap.amount_in.ensure_add(new_swap.amount_in)?;
					T::TokenSwaps::update_order(
						who.clone(),
						swap_id,
						amount_to_swap,
						T::BalanceRatio::one(),
					)?;

					Ok(SwapStatus {
						swapped: T::Balance::zero(),
						pending: amount_to_swap,
						swap_id: Some(swap_id),
					})
				} else {
					let inverse_swap = swap;

					let new_swap_amount_out = T::CurrencyConverter::stable_to_stable(
						new_swap.currency_out,
						new_swap.currency_in,
						new_swap.amount_in,
					)?;

					match inverse_swap.amount_in.cmp(&new_swap_amount_out) {
						Ordering::Greater => {
							let amount_to_swap =
								inverse_swap.amount_in.ensure_sub(new_swap_amount_out)?;

							T::TokenSwaps::update_order(
								who.clone(),
								swap_id,
								amount_to_swap,
								T::BalanceRatio::one(),
							)?;

							Ok(SwapStatus {
								swapped: new_swap.amount_in,
								pending: T::Balance::zero(),
								swap_id: Some(swap_id),
							})
						}
						Ordering::Equal => {
							T::TokenSwaps::cancel_order(swap_id)?;

							Ok(SwapStatus {
								swapped: new_swap.amount_in,
								pending: T::Balance::zero(),
								swap_id: None,
							})
						}
						Ordering::Less => {
							T::TokenSwaps::cancel_order(swap_id)?;

							let inverse_swap_amount_out = T::CurrencyConverter::stable_to_stable(
								inverse_swap.currency_out,
								inverse_swap.currency_in,
								inverse_swap.amount_in,
							)?;

							let amount_to_swap =
								new_swap.amount_in.ensure_sub(inverse_swap_amount_out)?;

							let swap_id = T::TokenSwaps::place_order(
								who.clone(),
								new_swap.currency_in,
								new_swap.currency_out,
								amount_to_swap,
								T::BalanceRatio::one(),
							)?;

							Ok(SwapStatus {
								swapped: inverse_swap_amount_out,
								pending: amount_to_swap,
								swap_id: Some(swap_id),
							})
						}
					}
				}
			}
		}
	}
}
