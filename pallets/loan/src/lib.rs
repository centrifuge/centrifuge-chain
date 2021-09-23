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
pub use pallet::*;
use pallet_nft::types::AssetId;
use pallet_registry::traits::VerifierRegistry;
use pallet_registry::types::{MintInfo, RegistryInfo};
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;
use sp_std::convert::TryInto;
use unique_assets::traits::{Mintable, Unique};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod math;

/// The data structure for storing loan info
#[derive(Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct LoanInfo<Rate, Amount, Moment, AssetId> {
	ceiling: Amount,
	borrowed_amount: Amount,
	rate_per_sec: Rate,
	accumulated_rate: Rate,
	normalised_debt: Amount,
	last_updated: Moment,
	asset_id: AssetId,
}

impl<Rate, Amount, Moment, AssetId> LoanInfo<Rate, Amount, Moment, AssetId>
where
	Amount: PartialOrd + sp_arithmetic::traits::Zero,
{
	/// returns true if the loan is active
	fn is_loan_active(&self) -> bool {
		self.borrowed_amount > Zero::zero()
	}
}

pub type RegistryIDOf<T> = <T as pallet_nft::Config>::RegistryId;
pub type TokenIdOf<T> = <T as pallet_nft::Config>::TokenId;
pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type AssetIdOf<T> = AssetId<RegistryIDOf<T>, TokenIdOf<T>>;
pub type AssetInfoOf<T> = <T as pallet_nft::Config>::AssetInfo;
type HashOf<T> = <T as frame_system::Config>::Hash;
pub type MintInfoOf<T> = MintInfo<HashOf<T>, HashOf<T>>;

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use sp_arithmetic::FixedPointNumber;
	use sp_core::U256;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_pool::Config + pallet_timestamp::Config + pallet_nft::Config
	{
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// the rate type
		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber;

		/// the amount type
		type Amount: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber;

		/// The nft registry trait that can mint, transfer and give owner details
		type NftRegistry: Unique<AssetIdOf<Self>, AccountIdOf<Self>>
			+ Mintable<AssetIdOf<Self>, AssetInfoOf<Self>, AccountIdOf<Self>>;

		/// Verifier registry to create NFT Registry
		/// TODO(ved): use simple registry instead of Va Registry when we have it
		type VaRegistry: VerifierRegistry<
			AccountIdOf<Self>,
			RegistryIDOf<Self>,
			RegistryInfo,
			AssetIdOf<Self>,
			AssetInfoOf<Self>,
			MintInfoOf<Self>,
		>;

		/// PalletID of this loan module
		#[pallet::constant]
		type LoanPalletId: Get<PalletId>;
	}

	/// Stores the loan nft registry ID against
	#[pallet::storage]
	#[pallet::getter(fn get_loan_nft_registry)]
	pub(super) type PoolToLoanNftRegistry<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolID, RegistryIDOf<T>, OptionQuery>;

	/// Stores the poolID with registryID as a key
	#[pallet::storage]
	pub(super) type LoanNftRegistryToPool<T: Config> =
		StorageMap<_, Blake2_128Concat, RegistryIDOf<T>, T::PoolID, OptionQuery>;

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
	pub(super) type Loan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolID,
		Blake2_128Concat,
		T::LoanID,
		LoanInfo<T::Rate, T::Amount, T::Moment, AssetIdOf<T>>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// emits when a new loan is issued for a given
		LoanIssued(T::PoolID, T::LoanID),

		/// emits when the loan info is updated.
		LoanInfoUpdate(T::PoolID, T::LoanID),

		/// emits when the loan is activated
		LoanActivated(T::PoolID, T::LoanID),

		/// emits when some amount is borrowed again
		LoanAmountBorrowed(T::PoolID, T::LoanID, T::Amount),
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the loan info for a given loan in a pool
		/// we update the loan details only if its not active
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn issue_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolID,
			asset_id: AssetIdOf<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let loan_id = Self::issue(pool_id, owner, asset_id)?;
			Self::deposit_event(Event::<T>::LoanIssued(pool_id, loan_id));
			Ok(())
		}

		/// Sets the loan info for a given loan in a pool
		/// we update the loan details only if its not active
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn update_loan_info(
			origin: OriginFor<T>,
			pool_id: T::PoolID,
			loan_id: T::LoanID,
			rate: T::Rate,
			principal: T::Amount,
		) -> DispatchResult {
			// TODO(dev): get the origin from the config. Admin can set loan information
			ensure_signed(origin)?;

			// check if the pool exists
			pallet_pool::Pallet::<T>::check_pool(pool_id)?;

			// check if the loan is active
			let loan_info = Loan::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;
			ensure!(!loan_info.is_loan_active(), Error::<T>::ErrLoanIsActive);

			// update the loan info
			Loan::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
				let mut loan_info = maybe_loan_info.take().unwrap_or_default();
				loan_info.rate_per_sec = rate;
				loan_info.ceiling = principal;
				*maybe_loan_info = Some(loan_info);
			});

			Self::deposit_event(Event::<T>::LoanInfoUpdate(pool_id, loan_id));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// returns the ceiling of the given loan under a given pool.
	pub fn ceiling(pool_id: T::PoolID, loan_id: T::LoanID) -> Option<T::Amount> {
		let maybe_loan_info = Loan::<T>::get(pool_id, loan_id);
		maybe_loan_info.map(|loan_info| loan_info.ceiling)
	}

	/// fetches the loan nft registry for a given pool. If missing, then will create one,
	/// update the state and returns the newly created nft registry
	fn fetch_or_create_loan_nft_registry_for_pool(pool_id: T::PoolID) -> T::RegistryId {
		match PoolToLoanNftRegistry::<T>::get(pool_id) {
			Some(registry_id) => registry_id,
			None => {
				let loan_pallet_id = T::LoanPalletId::get().into_account();

				// ensure owner can burn the nft when the loan is closed
				let registry_info = RegistryInfo {
					owner_can_burn: true,
					fields: vec![],
				};

				let registry_id = T::VaRegistry::create_new_registry(loan_pallet_id, registry_info);

				// update the storage
				PoolToLoanNftRegistry::<T>::insert(pool_id, registry_id);
				LoanNftRegistryToPool::<T>::insert(registry_id, pool_id);
				registry_id
			}
		}
	}

	/// issues a new loan nft and returns the LoanID
	fn issue(
		pool_id: T::PoolID,
		asset_owner: T::AccountId,
		asset_id: AssetIdOf<T>,
	) -> Result<T::LoanID, sp_runtime::DispatchError> {
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
		let timestamp = <pallet_timestamp::Pallet<T>>::get();
		let loan_id: T::LoanID = loan_nft_id.into();
		Loan::<T>::insert(
			pool_id,
			loan_id,
			LoanInfo {
				ceiling: Zero::zero(),
				borrowed_amount: Zero::zero(),
				rate_per_sec: Zero::zero(),
				accumulated_rate: Zero::zero(),
				normalised_debt: Zero::zero(),
				last_updated: timestamp,
				asset_id,
			},
		);
		Ok(loan_id)
	}

	pub fn borrow(pool_id: T::PoolID, loan_id: T::LoanID, amount: T::Amount) -> DispatchResult {
		let loan_info = Loan::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;

		ensure!(
			loan_info.ceiling <= amount + loan_info.borrowed_amount,
			Error::<T>::ErrLoanCeilingReached
		);

		let new_borrowed_amount = loan_info
			.borrowed_amount
			.checked_add(&amount)
			.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		let nowt = <pallet_timestamp::Pallet<T>>::get();
		let now: u64 = TryInto::<u64>::try_into(nowt).or(Err(Error::<T>::ErrEpochOverflow))?;
		let last_updated: u64 = TryInto::<u64>::try_into(loan_info.last_updated)
			.or(Err(Error::<T>::ErrEpochOverflow))?;
		let new_chi = math::calculate_cumulative_rate::<T::Rate>(
			loan_info.rate_per_sec,
			loan_info.accumulated_rate,
			now,
			last_updated,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		let debt =
			math::debt::<T::Amount, T::Rate>(loan_info.normalised_debt, loan_info.accumulated_rate)
				.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		let new_pie = math::calculate_normalised_debt::<T::Amount, T::Rate>(
			debt,
			math::Adjustment::Inc(amount),
			new_chi,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		Loan::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
			let mut loan_info = maybe_loan_info.take().unwrap_or_default();
			loan_info.borrowed_amount = new_borrowed_amount;
			loan_info.last_updated = nowt;
			loan_info.accumulated_rate = new_chi;
			loan_info.normalised_debt = new_pie;
			*maybe_loan_info = Some(loan_info);
		});

		if loan_info.borrowed_amount == Zero::zero() {
			Self::deposit_event(Event::<T>::LoanActivated(pool_id, loan_id));
		}

		Self::deposit_event(Event::<T>::LoanAmountBorrowed(pool_id, loan_id, amount));

		Ok(())
	}
}
