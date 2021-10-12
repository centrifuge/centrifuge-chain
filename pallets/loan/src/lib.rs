//! # Loan pallet for runtime
//!
//! This pallet provides functionality for managing loans on Tinlake
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::sp_runtime::traits::Zero;
use frame_support::transactional;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::CheckedAdd;
use std::fmt::Debug;

use frame_support::pallet_prelude::Get;
use frame_support::storage::types::OptionQuery;
use frame_support::traits::{EnsureOrigin, Time};
use frame_system::RawOrigin;
pub use pallet::*;
use pallet_nft::types::AssetId;
use pallet_registry::traits::VerifierRegistry;
use pallet_registry::types::{MintInfo, RegistryInfo};
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::DispatchError;
use sp_std::convert::TryInto;
use unique_assets::traits::{Mintable, Unique};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod math;

/// The data structure for storing loan info
#[derive(Encode, Decode, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub enum LoanStatus {
	Issued,
	Active,
	Closed,
}

/// The data structure for storing loan info
#[derive(Encode, Decode, Copy, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct LoanData<Rate, Amount, AssetId> {
	ceiling: Amount,
	borrowed_amount: Amount,
	rate_per_sec: Rate,
	accumulated_rate: Rate,
	principal_debt: Amount,
	last_updated: u64,
	maturity_date: Option<u64>,
	asset_id: AssetId,
	status: LoanStatus,
}

pub type RegistryIdOf<T> = <T as pallet_nft::Config>::RegistryId;
pub type TokenIdOf<T> = <T as pallet_nft::Config>::TokenId;
pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type AssetIdOf<T> = AssetId<RegistryIdOf<T>, TokenIdOf<T>>;
pub type AssetInfoOf<T> = <T as pallet_nft::Config>::AssetInfo;
type HashOf<T> = <T as frame_system::Config>::Hash;
pub type MintInfoOf<T> = MintInfo<HashOf<T>, HashOf<T>>;
pub type LoanIdOf<T> = <T as pallet_pool::Config>::LoanId;

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use pallet_pool::MultiCurrencyBalanceOf;
	use sp_arithmetic::FixedPointNumber;
	use sp_core::U256;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_pool::Config + pallet_nft::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// the rate type
		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber;

		/// the amount type
		type Amount: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ Into<MultiCurrencyBalanceOf<Self>>;

		/// The nft registry trait that can mint, transfer and give owner details
		type NftRegistry: Unique<AssetIdOf<Self>, AccountIdOf<Self>>
			+ Mintable<AssetIdOf<Self>, AssetInfoOf<Self>, AccountIdOf<Self>>;

		/// Verifier registry to create NFT Registry
		/// TODO(ved): migrate to Uniques pallet
		type VaRegistry: VerifierRegistry<
			AccountIdOf<Self>,
			RegistryIdOf<Self>,
			TokenIdOf<Self>,
			AssetInfoOf<Self>,
			HashOf<Self>,
		>;

		/// A way for use to fetch the time of the current blocks
		type Time: frame_support::traits::Time;

		/// PalletID of this loan module
		#[pallet::constant]
		type LoanPalletId: Get<PalletId>;
	}

	/// Stores the loan nft registry ID against
	#[pallet::storage]
	#[pallet::getter(fn get_loan_nft_registry)]
	pub(super) type PoolToLoanNftRegistry<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, RegistryIdOf<T>, OptionQuery>;

	/// Stores the poolID with registryID as a key
	#[pallet::storage]
	pub(super) type LoanNftRegistryToPool<T: Config> =
		StorageMap<_, Blake2_128Concat, RegistryIdOf<T>, T::PoolId, OptionQuery>;

	#[pallet::type_value]
	pub fn OnNextNftTokenIDEmpty() -> U256 {
		// always start the token ID from 1 instead of zero
		U256::one()
	}

	/// Stores the next loan tokenID to be issued
	#[pallet::storage]
	#[pallet::getter(fn get_next_loan_nft_token_id)]
	pub(super) type NextLoanNftTokenID<T: Config> =
		StorageValue<_, U256, ValueQuery, OnNextNftTokenIDEmpty>;

	/// Stores the loan info for given pool and loan id
	#[pallet::storage]
	#[pallet::getter(fn get_loan_info)]
	pub(super) type LoanInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::LoanId,
		LoanData<T::Rate, T::Amount, AssetIdOf<T>>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// emits when a new loan is issued for a given
		LoanIssued(T::PoolId, T::LoanId),

		/// emits when a loan is closed
		LoanClosed(T::PoolId, T::LoanId, AssetIdOf<T>),

		/// emits when the loan is activated
		LoanActivated(T::PoolId, T::LoanId),

		/// emits when some amount is borrowed
		LoanAmountBorrowed(T::PoolId, T::LoanId, T::Amount),

		/// emits when some amount is repaid
		LoanAmountRepaid(T::PoolId, T::LoanId, T::Amount),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when loan doesn't exist.
		ErrMissingLoan,

		/// Emits when the borrowed amount is more than ceiling
		ErrLoanCeilingReached,

		/// Emits when the addition of borrowed amount overflowed
		ErrAddBorrowedOverflow,

		/// Emits when the subtraction of ceiling amount under flowed
		ErrSubCeilingUnderflow,

		/// Emits when tries to update an active loan
		ErrLoanIsActive,

		/// Emits when epoch time is overflowed
		ErrEpochOverflow,

		/// Emits when the NFT owner is not found
		ErrNFTOwnerNotFound,

		/// Emits when nft owner doesn't match the expected owner
		ErrNotNFTOwner,

		/// Emits when the nft is not an acceptable asset
		ErrNotAValidAsset,

		/// Emits when the nft token nonce is overflowed
		ErrNftTokenNonceOverflowed,

		/// Emits when loan amount not repayed but trying to close loan
		ErrLoanNotRepaid,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Issues a new loan against the asset provided
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn issue_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			asset_id: AssetIdOf<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let loan_id = Self::issue(pool_id, owner, asset_id)?;
			Self::deposit_event(Event::<T>::LoanIssued(pool_id, loan_id));
			Ok(())
		}

		/// Closes a given loan if repaid fully
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn close_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let asset = Self::close(pool_id, loan_id, owner)?;
			Self::deposit_event(Event::<T>::LoanClosed(pool_id, loan_id, asset));
			Ok(())
		}

		/// borrows some amount from an active loan
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn borrow(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			amount: T::Amount,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::borrow_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::LoanAmountBorrowed(pool_id, loan_id, amount));
			Ok(())
		}

		/// repays some amount to an active loan
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			amount: T::Amount,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let repaid_amount = Self::repay_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::LoanAmountRepaid(
				pool_id,
				loan_id,
				repaid_amount,
			));
			Ok(())
		}

		/// a permissioned call to update loan specific details and activates the loan
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn activate_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			rate_per_sec: T::Rate,
			ceiling: T::Amount,
			maturity_date: Option<u64>,
		) -> DispatchResult {
			// TODO(dev): get the origin from the config. Admin can set loan information
			ensure_signed(origin)?;

			// check if the pool exists
			pallet_pool::Pallet::<T>::check_pool(pool_id)?;

			// ensure loan is in issued state
			let loan_info =
				LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;
			ensure!(
				loan_info.status == LoanStatus::Issued,
				Error::<T>::ErrLoanIsActive
			);

			// update the loan info
			LoanInfo::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
				let mut loan_info = maybe_loan_info.take().unwrap();
				loan_info.rate_per_sec = rate_per_sec;
				loan_info.ceiling = ceiling;
				loan_info.status = LoanStatus::Active;
				loan_info.maturity_date = maturity_date;
				*maybe_loan_info = Some(loan_info);
			});

			Self::deposit_event(Event::<T>::LoanActivated(pool_id, loan_id));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// returns the account_id of the loan pallet
	pub fn account_id() -> T::AccountId {
		T::LoanPalletId::get().into_account()
	}

	/// returns the ceiling of the given loan under a given pool.
	pub fn ceiling(pool_id: T::PoolId, loan_id: T::LoanId) -> Option<T::Amount> {
		let maybe_loan_info = LoanInfo::<T>::get(pool_id, loan_id);
		maybe_loan_info.map(|loan_info| loan_info.ceiling)
	}

	/// fetches the loan nft registry for a given pool. If missing, then will create one,
	/// update the state and returns the newly created nft registry
	fn fetch_or_create_loan_nft_registry_for_pool(pool_id: T::PoolId) -> T::RegistryId {
		match PoolToLoanNftRegistry::<T>::get(pool_id) {
			Some(registry_id) => registry_id,
			None => {
				let loan_pallet_id = Self::account_id();

				// ensure owner can burn the nft when the loan is closed
				let registry_info = RegistryInfo {
					owner_can_burn: true,
					fields: vec![],
				};

				let registry_id =
					T::VaRegistry::create_new_registry(loan_pallet_id.into(), registry_info);

				// update the storage
				PoolToLoanNftRegistry::<T>::insert(pool_id, registry_id);
				LoanNftRegistryToPool::<T>::insert(registry_id, pool_id);
				registry_id
			}
		}
	}

	/// check if the given loan belongs to the owner provided
	fn check_loan_owner(
		pool_id: T::PoolId,
		loan_id: T::LoanId,
		owner: T::AccountId,
	) -> Result<AssetIdOf<T>, DispatchError> {
		let registry_id = Self::fetch_or_create_loan_nft_registry_for_pool(pool_id);
		let got = T::NftRegistry::owner_of(AssetId(registry_id, loan_id.into()))
			.ok_or(Error::<T>::ErrNotNFTOwner)?;
		ensure!(got == owner, Error::<T>::ErrNotNFTOwner);
		Ok(AssetId(registry_id, loan_id.into()))
	}

	/// issues a new loan nft and returns the LoanID
	fn issue(
		pool_id: T::PoolId,
		asset_owner: T::AccountId,
		asset_id: AssetIdOf<T>,
	) -> Result<T::LoanId, sp_runtime::DispatchError> {
		// check if the nft belongs to owner
		let owner = T::NftRegistry::owner_of(asset_id).ok_or(Error::<T>::ErrNFTOwnerNotFound)?;
		ensure!(owner == asset_owner, Error::<T>::ErrNotNFTOwner);

		// check if the registry is not an loan nft registry
		ensure!(
			!LoanNftRegistryToPool::<T>::contains_key(asset_id.0),
			Error::<T>::ErrNotAValidAsset
		);

		// create new loan nft
		let loan_pallet_account: AccountIdOf<T> = T::LoanPalletId::get().into_account();
		let token_nonce = NextLoanNftTokenID::<T>::get();
		let loan_nft_id: T::TokenId = token_nonce.into();
		let loan_nft_registry = Self::fetch_or_create_loan_nft_registry_for_pool(pool_id);
		let loan_asset_id = AssetId(loan_nft_registry, loan_nft_id);
		let asset_info = Default::default();
		T::NftRegistry::mint(
			loan_pallet_account.clone(),
			owner,
			loan_asset_id,
			asset_info,
		)?;

		// update the next token nonce
		let next_token_id = token_nonce
			.checked_add(U256::one())
			.ok_or(Error::<T>::ErrNftTokenNonceOverflowed)?;
		NextLoanNftTokenID::<T>::set(next_token_id);

		// lock asset nft
		T::NftRegistry::transfer(asset_owner, loan_pallet_account, asset_id)?;
		let timestamp = Self::time_now()?;
		let loan_id: T::LoanId = loan_nft_id.into();
		LoanInfo::<T>::insert(
			pool_id,
			loan_id,
			LoanData {
				ceiling: Zero::zero(),
				borrowed_amount: Zero::zero(),
				rate_per_sec: Zero::zero(),
				accumulated_rate: Zero::zero(),
				principal_debt: Zero::zero(),
				last_updated: timestamp,
				maturity_date: None,
				asset_id,
				status: LoanStatus::Issued,
			},
		);
		Ok(loan_id)
	}

	fn close(
		pool_id: T::PoolId,
		loan_id: T::LoanId,
		owner: T::AccountId,
	) -> Result<AssetIdOf<T>, DispatchError> {
		// ensure owner is the loan nft owner
		let loan_nft = Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		// TODO(ved): ensure loan is active

		// ensure debt is all paid
		let mut loan_info =
			LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;
		ensure!(
			loan_info.principal_debt == Zero::zero(),
			Error::<T>::ErrLoanNotRepaid
		);

		// transfer asset to owner
		let asset = loan_info.asset_id;
		T::NftRegistry::transfer(Self::account_id(), owner.clone(), asset)?;

		// transfer loan nft to loan pallet
		// ideally we should burn this but we do not have a function to burn them yet.
		// TODO(ved): burn loan nft when the functionality is available
		T::NftRegistry::transfer(owner, Self::account_id(), loan_nft)?;

		// update loan status
		loan_info.status = LoanStatus::Closed;
		LoanInfo::<T>::insert(pool_id, loan_id, loan_info);
		Ok(asset)
	}

	fn borrow_amount(
		pool_id: T::PoolId,
		loan_id: T::LoanId,
		owner: T::AccountId,
		amount: T::Amount,
	) -> DispatchResult {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		// TODO(ved): ensure loan is active
		// fetch the loan details and check for ceiling threshold
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;
		ensure!(
			loan_info.ceiling <= amount + loan_info.borrowed_amount,
			Error::<T>::ErrLoanCeilingReached
		);

		let new_borrowed_amount = loan_info
			.borrowed_amount
			.checked_add(&amount)
			.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		// calculate new accumulated rate
		let now: u64 = Self::time_now()?;
		let accumulated_rate = math::calculate_accumulated_rate::<T::Rate>(
			loan_info.rate_per_sec,
			loan_info.accumulated_rate,
			now,
			loan_info.last_updated,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		// calculate current debt
		let debt = math::debt::<T::Amount, T::Rate>(loan_info.principal_debt, accumulated_rate)
			.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		// calculate new principal debt with borrowed amount
		let principal_debt = math::calculate_principal_debt::<T::Amount, T::Rate>(
			debt,
			math::Adjustment::Inc(amount),
			accumulated_rate,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		LoanInfo::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
			let mut loan_info = maybe_loan_info.take().unwrap();
			loan_info.borrowed_amount = new_borrowed_amount;
			loan_info.last_updated = now;
			loan_info.accumulated_rate = accumulated_rate;
			loan_info.principal_debt = principal_debt;
			*maybe_loan_info = Some(loan_info);
		});

		pallet_pool::Pallet::<T>::borrow_currency(
			pool_id,
			RawOrigin::Signed(Self::account_id()).into(),
			owner,
			amount.into(),
		)?;
		Ok(())
	}

	fn repay_amount(
		pool_id: T::PoolId,
		loan_id: T::LoanId,
		owner: T::AccountId,
		amount: T::Amount,
	) -> Result<T::Amount, DispatchError> {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		// fetch the loan details
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;

		// TODO(ved): ensure loan is active
		// calculate new accumulated rate
		let now: u64 = Self::time_now()?;
		let accumulated_rate = math::calculate_accumulated_rate::<T::Rate>(
			loan_info.rate_per_sec,
			loan_info.accumulated_rate,
			now,
			loan_info.last_updated,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		// calculate current debt
		let debt = math::debt::<T::Amount, T::Rate>(loan_info.principal_debt, accumulated_rate)
			.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		// ensure amount is not more than current debt
		let mut repay_amount = amount;
		if repay_amount > debt {
			repay_amount = debt
		}

		// calculate new principal debt with repayed amount
		let principal_debt = math::calculate_principal_debt::<T::Amount, T::Rate>(
			debt,
			math::Adjustment::Dec(repay_amount),
			accumulated_rate,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		LoanInfo::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
			let mut loan_info = maybe_loan_info.take().unwrap();
			loan_info.last_updated = now;
			loan_info.accumulated_rate = accumulated_rate;
			loan_info.principal_debt = principal_debt;
			*maybe_loan_info = Some(loan_info);
		});

		pallet_pool::Pallet::<T>::repay_currency(
			pool_id,
			RawOrigin::Signed(Self::account_id()).into(),
			owner,
			repay_amount.into(),
		)?;
		Ok(repay_amount)
	}

	fn time_now() -> Result<u64, DispatchError> {
		let nowt = T::Time::now();
		TryInto::<u64>::try_into(nowt).map_err(|_| Error::<T>::ErrEpochOverflow.into())
	}
}

/// Simple ensure origin for the loan account
pub struct EnsureLoanAccount<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet::Config> EnsureOrigin<T::Origin> for EnsureLoanAccount<T> {
	type Success = T::AccountId;

	fn try_origin(o: T::Origin) -> Result<Self::Success, T::Origin> {
		let loan_id = T::LoanPalletId::get().into_account();
		o.into().and_then(|o| match o {
			RawOrigin::Signed(who) if who == loan_id => Ok(loan_id),
			r => Err(T::Origin::from(r)),
		})
	}
}
