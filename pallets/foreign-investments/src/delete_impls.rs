// TODO: How to merge token swaps and investment trait? Create new trait
// ForeignInvestment? > Check diagrams

// impl<T: Config> Investment<T::AccountId> for Pallet<T> {
// 	type Amount = T::Balance;
// 	type CurrencyId = T::CurrencyId;
// 	type Error = DispatchError;
// 	type InvestmentId = T::InvestmentId;

// 	fn update_investment(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 		amount: Self::Amount,
// 	) -> Result<(), Self::Error> {
// 		let pre_amount = Self::investment(who, investment_id.clone())?;
// 		let pre_state = InvestmentState::<T>::get(who,
// investment_id.clone()).unwrap_or_default();

// 		if amount > pre_amount {
// 			// TODO: Can payment currency be derived?
// 			let swap_currency =
// 				<Self as Accountant>::info(investment_id).map(|info|
// info.payment_currency()); 			let post_state: Option<InvestState<<T as
// Config>::Balance, <T as Config>::CurrencyId>> = 				pre_state.
// transition(InvestTransition::IncreaseInvestOrder(amount))?; 			Ok(())
// 		} else if amount < pre_amount {
// 			let post_state: Option<InvestState<<T as Config>::Balance, <T as
// Config>::CurrencyId>> = 				pre_state.
// transition(InvestTransition::DecreaseInvestOrder(amount))?; 			Ok(())
// 		} else {
// 			Ok(())
// 		}
// 	}

// 	fn accepted_payment_currency(
// 		investment_id: Self::InvestmentId,
// 		currency: Self::CurrencyId,
// 	) -> bool {
// 		T::Investment::accepted_payment_currency(investment_id, currency)
// 	}

// 	fn investment(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 	) -> Result<Self::Amount, Self::Error> {
// 		todo!()
// 	}

// 	fn update_redemption(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 		amount: Self::Amount,
// 	) -> Result<(), Self::Error> {
// 		todo!()
// 	}

// 	fn accepted_payout_currency(
// 		investment_id: Self::InvestmentId,
// 		currency: Self::CurrencyId,
// 	) -> bool {
// 		T::Investment::accepted_payout_currency(investment_id, currency)
// 	}

// 	fn redemption(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 	) -> Result<Self::Amount, Self::Error> {
// 		todo!()
// 	}
// }
