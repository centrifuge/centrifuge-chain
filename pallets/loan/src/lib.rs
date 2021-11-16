//! # Loan pallet for runtime
//!
//! This pallet provides functionality for managing loans on Tinlake
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::sp_runtime::traits::{One, Zero};
use frame_support::transactional;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{CheckedAdd, CheckedSub};
use std::fmt::Debug;

use common_traits::PoolNAV as TPoolNav;
use frame_support::pallet_prelude::Get;
use frame_support::storage::types::OptionQuery;
use frame_support::traits::{EnsureOrigin, Time};
use frame_system::RawOrigin;
use loan_type::LoanType;
pub use pallet::*;
use pallet_nft::types::AssetId;
use pallet_registry::traits::VerifierRegistry;
use pallet_registry::types::{MintInfo, RegistryInfo};
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{DispatchError, FixedPointNumber};
use sp_std::convert::TryInto;
use unique_assets::traits::{Mintable, Unique};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod loan_type;
pub mod math;

/// The data structure for storing pool nav details
#[derive(Encode, Decode, Copy, Clone, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct NAVDetails<Amount> {
	// this is the latest nav for the given pool.
	// this will be updated on these scenarios
	// 1. When we are calculating pool nav
	// 2. when there is borrow or repay or write off on a loan under this pool
	// So NAV could be
	//	approximate when current time != last_updated
	//	exact when current time == last_updated
	latest_nav: Amount,

	// this is the last time when the nav was calculated for the entire pool
	last_updated: u64,
}

/// The data structure for storing a specific write off group
#[derive(Encode, Decode, Copy, Clone, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct WriteOffGroup<Rate> {
	/// percentage of outstanding debt we are going to write off on a loan
	percentage: Rate,

	/// number in days after the maturity has passed at which this write off group is valid
	overdue_days: u64,
}

/// The data structure for storing loan info
#[derive(Encode, Decode, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub enum LoanStatus {
	// this when asset is locked and loan nft is issued.
	Issued,
	// this is when loan is in active state. Either underwriters or oracles can move loan to this state
	// by providing information like discount rates etc.. to loan
	Active,
	// loan is closed and asset nft is transferred back to borrower and loan nft is transferred back to loan module
	Closed,
}

/// The data structure for storing loan info
#[derive(Encode, Decode, Copy, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct LoanData<Rate, Amount, AssetId> {
	ceiling: Amount,
	borrowed_amount: Amount,
	rate_per_sec: Rate,
	// accumulated rate till last_updated. more about this here - https://docs.makerdao.com/smart-contract-modules/rates-module
	accumulated_rate: Rate,
	// principal debt used to calculate the current outstanding debt.
	// principal debt will change on every borrow and repay.
	// Called principal debt instead of pie or normalized debt as mentioned here - https://docs.makerdao.com/smart-contract-modules/rates-module
	// since its easier to look at it as principal amount borrowed and can be used to calculate final debt with interest rate
	principal_debt: Amount,
	last_updated: u64,
	asset_id: AssetId,
	status: LoanStatus,
	loan_type: LoanType<Rate, Amount>,

	// whether the loan written off by admin
	// if so, we wont update the write off group on this loan further from permission less call
	admin_written_off: bool,
	// write off group index in the vec of write off groups
	// none, the loan is not written off yet
	// some(index), loan is written off and write off details are found under the given index
	write_off_index: Option<u32>,
}

impl<Rate, Amount, AssetID> LoanData<Rate, Amount, AssetID>
where
	Rate: FixedPointNumber,
	Amount: FixedPointNumber,
{
	/// returns the present value of the loan
	/// note: this will use the accumulated_rate and last_updated from self
	/// if you want the latest upto date present value, ensure these values are updated as well before calling this
	fn present_value(&self) -> Option<Amount> {
		// calculate current debt and present value
		math::debt(self.principal_debt, self.accumulated_rate).and_then(|debt| {
			self.loan_type
				.present_value(debt, self.last_updated, self.rate_per_sec)
		})
	}

	/// accrues rate and current debt from last updated until now
	fn accrue(&self, now: u64) -> Option<(Rate, Amount)> {
		// if the borrow amount is zero, then set accumulated rate to rate per sec so we start accumulating from now.
		let maybe_rate = match self.borrowed_amount == Zero::zero() {
			true => Some(self.rate_per_sec),
			false => math::calculate_accumulated_rate::<Rate>(
				self.rate_per_sec,
				self.accumulated_rate,
				now,
				self.last_updated,
			),
		};

		// calculate the current outstanding debt
		let maybe_debt = maybe_rate
			.and_then(|acc_rate| math::debt::<Amount, Rate>(self.principal_debt, acc_rate));

		match (maybe_rate, maybe_debt) {
			(Some(rate), Some(debt)) => Some((rate, debt)),
			_ => None,
		}
	}

	/// returns the present value of the loan adjusted to the write off group assigned to the loan if any
	// pv = pv*(1 - write_off_percentage)
	fn present_value_with_write_off(
		&self,
		write_off_groups: Vec<WriteOffGroup<Rate>>,
	) -> Option<Amount> {
		let maybe_present_value = self.present_value();
		match self.write_off_index {
			None => maybe_present_value,
			Some(index) => maybe_present_value.and_then(|pv| {
				write_off_groups
					.get(index as usize)
					// convert rate to amount
					.and_then(|group| math::convert::<Rate, Amount>(group.percentage))
					// calculate write off amount
					.and_then(|write_off_percentage| pv.checked_mul(&write_off_percentage))
					// calculate adjusted present value
					.and_then(|write_off_amount| pv.checked_sub(&write_off_amount))
			}),
		}
	}
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

		/// A way for use to fetch the time of the current block
		type Time: frame_support::traits::Time;

		/// PalletID of this loan module
		#[pallet::constant]
		type LoanPalletId: Get<PalletId>;

		/// Origin for admin that can activate a loan
		type AdminOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
	}

	/// Stores the loan nft registry ID against
	#[pallet::storage]
	#[pallet::getter(fn get_loan_nft_registry)]
	pub(crate) type PoolToLoanNftRegistry<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, RegistryIdOf<T>, OptionQuery>;

	/// Stores the poolID with registryID as a key
	#[pallet::storage]
	pub(crate) type LoanNftRegistryToPool<T: Config> =
		StorageMap<_, Blake2_128Concat, RegistryIdOf<T>, T::PoolId, OptionQuery>;

	#[pallet::type_value]
	pub fn OnNextNftTokenIDEmpty() -> U256 {
		// always start the token ID from 1 instead of zero
		U256::one()
	}

	/// Stores the next loan tokenID to be issued
	#[pallet::storage]
	#[pallet::getter(fn get_next_loan_nft_token_id)]
	pub(crate) type NextLoanNftTokenID<T: Config> =
		StorageValue<_, U256, ValueQuery, OnNextNftTokenIDEmpty>;

	/// Stores the loan info for given pool and loan id
	#[pallet::storage]
	#[pallet::getter(fn get_loan_info)]
	pub(crate) type LoanInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::LoanId,
		LoanData<T::Rate, T::Amount, AssetIdOf<T>>,
		OptionQuery,
	>;

	/// Stores the pool nav against poolId
	#[pallet::storage]
	#[pallet::getter(fn nav)]
	pub(crate) type PoolNAV<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, NAVDetails<T::Amount>, OptionQuery>;

	/// Stores the pool associated with the its write off groups
	#[pallet::storage]
	#[pallet::getter(fn pool_writeoff_groups)]
	pub(crate) type PoolWriteOffGroups<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, Vec<WriteOffGroup<T::Rate>>, ValueQuery>;

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

		/// Emits when NAV is updated for a given pool
		NAVUpdated(T::PoolId, T::Amount),

		/// Emits when a write off group is added to the given pool with its index
		WriteOffGroupAdded(T::PoolId, u32),

		/// Emits when a loan is written off
		LoanWrittenOff(T::PoolId, T::LoanId, u32),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when loan doesn't exist.
		ErrMissingLoan,

		/// Emits when the borrowed amount is more than ceiling
		ErrLoanCeilingReached,

		/// Emits when the addition of borrowed amount overflowed
		ErrAddAmountOverflow,

		/// Emits when principal debt calculation failed due to overflow
		ErrPrincipalDebtOverflow,

		/// Emits when tries to update an active loan
		ErrLoanIsActive,

		/// Emits when loan type given is not valid
		ErrLoanTypeInvalid,

		/// Emits when operation is done on an inactive loan
		ErrLoanNotActive,

		/// Emits when epoch time is overflowed
		ErrEpochTimeOverflow,

		/// Emits when the NFT owner is not found
		ErrNFTOwnerNotFound,

		/// Emits when nft owner doesn't match the expected owner
		ErrNotNFTOwner,

		/// Emits when the nft is not an acceptable asset
		ErrNotAValidAsset,

		/// Emits when the nft token nonce is overflowed
		ErrNftTokenNonceOverflowed,

		/// Emits when loan amount not repaid but trying to close loan
		ErrLoanNotRepaid,

		/// Emits when maturity has passed and borrower tried to borrow more
		ErrLoanMaturityDatePassed,

		/// Emits when a loan data value is invalid
		ErrLoanValueInvalid,

		/// Emits when loan accrue calculation failed
		ErrLoanAccrueFailed,

		/// Emits when loan present value calculation failed
		ErrLoanPresentValueFailed,

		/// Emits when trying to write off of a healthy loan
		ErrLoanHealthy,

		/// Emits when trying to write off loan that was written off by admin already
		ErrLoanWrittenOffByAdmin,

		/// Emits when there is no valid write off group available for unhealthy loan
		ErrNoValidWriteOffGroup,

		/// Emits when there is no valid write off groups associated with given index
		ErrInvalidWriteOffGroupIndex,
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

		/// a call to update loan specific details and activates the loan
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn activate_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			rate_per_sec: T::Rate,
			loan_type: LoanType<T::Rate, T::Amount>,
		) -> DispatchResult {
			<T as Config>::AdminOrigin::ensure_origin(origin)?;

			// check if the pool exists
			pallet_pool::Pallet::<T>::check_pool(pool_id)?;

			// ensure loan is in issued state
			let loan_info =
				LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;
			ensure!(
				loan_info.status == LoanStatus::Issued,
				Error::<T>::ErrLoanIsActive
			);

			// calculate ceiling
			let ceiling = loan_type.ceiling().ok_or(Error::<T>::ErrLoanTypeInvalid)?;
			ensure!(ceiling > Zero::zero(), Error::<T>::ErrLoanValueInvalid);

			// ensure rate_per_sec >= one
			ensure!(rate_per_sec >= One::one(), Error::<T>::ErrLoanValueInvalid);

			// ensure loan_type is valid
			let now = Self::time_now()?;
			ensure!(loan_type.is_valid(now), Error::<T>::ErrLoanValueInvalid);

			// update the loan info
			LoanInfo::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
				let mut loan_info = maybe_loan_info.take().unwrap();
				loan_info.rate_per_sec = rate_per_sec;
				loan_info.ceiling = ceiling;
				loan_info.status = LoanStatus::Active;
				loan_info.loan_type = loan_type;
				*maybe_loan_info = Some(loan_info);
			});

			Self::deposit_event(Event::<T>::LoanActivated(pool_id, loan_id));
			Ok(())
		}

		/// a call to update nav for a given pool
		/// TODO(ved): benchmarking this to get a weight would be tricky due to n loans per pool
		/// Maybe utility pallet would be a good source of inspiration?
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn update_nav(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			// ensure signed so that caller pays for the update fees
			ensure_signed(origin)?;
			let updated_nav = Self::update_nav_of_pool(pool_id)?;
			Self::deposit_event(Event::<T>::NAVUpdated(pool_id, updated_nav));
			Ok(())
		}

		/// a call to add a new write off group for a given pool
		/// write off groups are always append only
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn add_write_off_group(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			group: WriteOffGroup<T::Rate>,
		) -> DispatchResult {
			// ensure this is coming from an admin origin
			<T as Config>::AdminOrigin::ensure_origin(origin)?;

			// check if the pool exists
			pallet_pool::Pallet::<T>::check_pool(pool_id)?;

			// append new group
			let index = PoolWriteOffGroups::<T>::mutate(pool_id, |write_off_groups| -> u32 {
				write_off_groups.push(group);
				// return the index of the write off group
				(write_off_groups.len() - 1) as u32
			});
			Self::deposit_event(Event::<T>::WriteOffGroupAdded(pool_id, index));
			Ok(())
		}

		/// a call to write off an unhealthy loan
		/// a valid write off group will be chosen based on the loan overdue date since maturity
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn write_off_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
		) -> DispatchResult {
			// ensure this is a signed call
			ensure_signed(origin)?;

			// try to write off
			let index = Self::write_off(pool_id, loan_id, None)?;
			Self::deposit_event(Event::<T>::LoanWrittenOff(pool_id, loan_id, index));
			Ok(())
		}

		/// a permissioned call to write off an unhealthy loan
		/// write_off_index is overwritten to the loan and the is fixed until changes it with another call.
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn admin_write_off_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			write_off_index: u32,
		) -> DispatchResult {
			// ensure this is a call from admin
			<T as Config>::AdminOrigin::ensure_origin(origin)?;

			// try to write off
			let index = Self::write_off(pool_id, loan_id, Some(write_off_index))?;
			Self::deposit_event(Event::<T>::LoanWrittenOff(pool_id, loan_id, index));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// returns the account_id of the loan pallet
	pub fn account_id() -> T::AccountId {
		T::LoanPalletId::get().into_account()
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
			.ok_or(Error::<T>::ErrNFTOwnerNotFound)?;
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
				accumulated_rate: One::one(),
				principal_debt: Zero::zero(),
				last_updated: timestamp,
				asset_id,
				status: LoanStatus::Issued,
				loan_type: Default::default(),
				admin_written_off: false,
				write_off_index: None,
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

		let mut loan_info =
			LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;

		// ensure loan is active
		ensure!(
			loan_info.status == LoanStatus::Active,
			Error::<T>::ErrLoanNotActive
		);

		// ensure debt is all paid
		// we just need to ensure principal debt is zero
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

		// fetch the loan details
		let loan_data = LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;

		// ensure loan is active
		ensure!(
			loan_data.status == LoanStatus::Active,
			Error::<T>::ErrLoanNotActive
		);

		// ensure maturity date has not passed if the loan has a maturity date
		let now: u64 = Self::time_now()?;
		let valid = match loan_data.loan_type.maturity_date() {
			// loan has a maturity date
			Some(md) => md > now,
			// no maturity date, so continue as is
			None => true,
		};
		ensure!(valid, Error::<T>::ErrLoanMaturityDatePassed);

		// ensure borrow amount is positive
		ensure!(amount.is_positive(), Error::<T>::ErrLoanValueInvalid);

		// check for ceiling threshold
		ensure!(
			amount + loan_data.borrowed_amount <= loan_data.ceiling,
			Error::<T>::ErrLoanCeilingReached
		);

		// get previous present value so that we can update the nav accordingly
		let old_pv = loan_data
			.present_value()
			.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

		// calculate accumulated rate and outstanding debt
		let (accumulated_rate, debt) = loan_data
			.accrue(now)
			.ok_or(Error::<T>::ErrLoanAccrueFailed)?;

		let new_borrowed_amount = loan_data
			.borrowed_amount
			.checked_add(&amount)
			.ok_or(Error::<T>::ErrAddAmountOverflow)?;

		// calculate new principal debt with adjustment amount
		let principal_debt = math::calculate_principal_debt::<T::Amount, T::Rate>(
			debt,
			math::Adjustment::Inc(amount),
			accumulated_rate,
		)
		.ok_or(Error::<T>::ErrPrincipalDebtOverflow)?;

		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_info| -> Result<(), DispatchError> {
				// unwrap since we already checked above
				let mut loan_data = maybe_loan_info.take().expect("loan data should be present");
				loan_data.borrowed_amount = new_borrowed_amount;
				loan_data.last_updated = now;
				loan_data.accumulated_rate = accumulated_rate;
				loan_data.principal_debt = principal_debt;
				let new_pv = loan_data
					.present_value()
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;
				pallet_pool::Pallet::<T>::borrow_currency(
					pool_id,
					RawOrigin::Signed(Self::account_id()).into(),
					owner,
					amount.into(),
				)?;
				*maybe_loan_info = Some(loan_data);
				Ok(())
			},
		)?;
		Ok(())
	}

	fn update_nav_with_updated_present_value(
		pool_id: T::PoolId,
		new_pv: T::Amount,
		old_pv: T::Amount,
	) -> Result<(), DispatchError> {
		// calculate new diff from the old and new present value and update the nav accordingly
		PoolNAV::<T>::try_mutate(pool_id, |maybe_nav_details| -> Result<(), DispatchError> {
			let mut nav = maybe_nav_details.take().unwrap_or_default();
			let new_nav = match new_pv > old_pv {
				// borrow
				true => new_pv
					.checked_sub(&old_pv)
					.and_then(|positive_diff| nav.latest_nav.checked_add(&positive_diff)),
				// repay since new pv is less than old
				false => old_pv
					.checked_sub(&new_pv)
					.and_then(|negative_diff| nav.latest_nav.checked_sub(&negative_diff)),
			}
			.ok_or(Error::<T>::ErrAddAmountOverflow)?;
			nav.latest_nav = new_nav;
			*maybe_nav_details = Some(nav);
			Ok(())
		})
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
		let loan_data = LoanInfo::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;

		// ensure loan is active
		ensure!(
			loan_data.status == LoanStatus::Active,
			Error::<T>::ErrLoanNotActive
		);

		// ensure repay amount is positive
		ensure!(amount.is_positive(), Error::<T>::ErrLoanValueInvalid);

		// calculate old present_value
		let old_pv = loan_data
			.present_value()
			.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

		// calculate new accumulated rate
		let now: u64 = Self::time_now()?;
		let (accumulated_rate, debt) = loan_data
			.accrue(now)
			.ok_or(Error::<T>::ErrLoanAccrueFailed)?;

		// ensure amount is not more than current debt
		let mut repay_amount = amount;
		if repay_amount > debt {
			repay_amount = debt
		}

		// calculate new principal debt with repaid amount
		let principal_debt = math::calculate_principal_debt::<T::Amount, T::Rate>(
			debt,
			math::Adjustment::Dec(repay_amount),
			accumulated_rate,
		)
		.ok_or(Error::<T>::ErrAddAmountOverflow)?;

		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_info| -> Result<(), DispatchError> {
				let mut loan_data = maybe_loan_info.take().expect("loan data should be present");
				loan_data.last_updated = now;
				loan_data.accumulated_rate = accumulated_rate;
				loan_data.principal_debt = principal_debt;
				let new_pv = loan_data
					.present_value()
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;
				pallet_pool::Pallet::<T>::repay_currency(
					pool_id,
					RawOrigin::Signed(Self::account_id()).into(),
					owner,
					repay_amount.into(),
				)?;
				*maybe_loan_info = Some(loan_data);
				Ok(())
			},
		)?;

		Ok(repay_amount)
	}

	fn time_now() -> Result<u64, DispatchError> {
		let nowt = T::Time::now();
		TryInto::<u64>::try_into(nowt).map_err(|_| Error::<T>::ErrEpochTimeOverflow.into())
	}

	/// accrues rate and debt of a given loan and updates it
	/// returns the present value of the loan
	fn accrue_and_update_loan(
		pool_id: T::PoolId,
		loan_id: T::LoanId,
		now: u64,
	) -> Result<T::Amount, DispatchError> {
		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_data| -> Result<T::Amount, DispatchError> {
				let mut loan_data = maybe_loan_data.take().ok_or(Error::<T>::ErrMissingLoan)?;
				// if the loan is not active, then skip updating and return PV as zero
				if loan_data.status != LoanStatus::Active {
					*maybe_loan_data = Some(loan_data);
					return Ok(Zero::zero());
				}

				let (acc_rate, _debt) = loan_data
					.accrue(now)
					.ok_or(Error::<T>::ErrLoanAccrueFailed)?;
				loan_data.last_updated = now;
				loan_data.accumulated_rate = acc_rate;
				let present_value = loan_data
					.present_value()
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;
				*maybe_loan_data = Some(loan_data);
				Ok(present_value)
			},
		)
	}

	/// updates nav for the given pool and returns the latest NAV at this instant
	fn update_nav_of_pool(pool_id: T::PoolId) -> Result<T::Amount, DispatchError> {
		let now = Self::time_now()?;
		let nav = LoanInfo::<T>::iter_key_prefix(pool_id).try_fold(
			Zero::zero(),
			|sum, loan_id| -> Result<T::Amount, DispatchError> {
				let pv = Self::accrue_and_update_loan(pool_id, loan_id, now)?;
				sum.checked_add(&pv)
					.ok_or(Error::<T>::ErrLoanAccrueFailed.into())
			},
		)?;
		PoolNAV::<T>::insert(
			pool_id,
			NAVDetails {
				latest_nav: nav,
				last_updated: now,
			},
		);
		Ok(nav)
	}

	/// writes off a given unhealthy loan
	/// if override_write_off_index is Some, this is a admin action and loan override flag is set
	/// if loan is already overridden and override_write_off_index is None, we return error
	/// if loan is still healthy, we return an error
	/// loan is accrued and nav is updated accordingly
	/// returns new write off index applied to loan
	fn write_off(
		pool_id: T::PoolId,
		loan_id: T::LoanId,
		override_write_off_index: Option<u32>,
	) -> Result<u32, DispatchError> {
		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_data| -> Result<u32, DispatchError> {
				let mut loan_data = maybe_loan_data.take().ok_or(Error::<T>::ErrMissingLoan)?;
				// ensure loan is active
				ensure!(
					loan_data.status == LoanStatus::Active,
					Error::<T>::ErrLoanNotActive
				);

				let maturity_date = loan_data
					.loan_type
					.maturity_date()
					.ok_or(Error::<T>::ErrLoanTypeInvalid)?;
				let now = Self::time_now()?;

				// ensure loan's maturity date has passed
				ensure!(now > maturity_date, Error::<T>::ErrLoanHealthy);

				// ensure loan was not overwritten by admin and try to fetch a valid write off group for loan
				let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
				let write_off_group_index =
					match (loan_data.admin_written_off, override_write_off_index) {
						// admin is trying to write off
						(_admin_written_off, Some(index)) => {
							// check if the write off group exists
							write_off_groups
								.get(index as usize)
								.ok_or(Error::<T>::ErrInvalidWriteOffGroupIndex)?;
							loan_data.admin_written_off = true;
							Ok(index)
						}
						(admin_written_off, None) => {
							// non-admin is trying to write off but admin already did. So error out
							if admin_written_off {
								return Err(Error::<T>::ErrLoanWrittenOffByAdmin.into());
							}

							// not written off by admin, and non admin trying to write off, then
							// fetch the best write group available for this loan
							math::valid_write_off_group(
								maturity_date,
								now,
								write_off_groups.clone(),
							)
							.ok_or(Error::<T>::ErrNoValidWriteOffGroup)
						}
					}?;

				// get old present value accounting for any write offs
				let old_pv = loan_data
					.present_value_with_write_off(write_off_groups.clone())
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

				// accrue and calculate the new present value with current chosen write off
				let (accumulated_rate, _current_debt) = loan_data
					.accrue(now)
					.ok_or(Error::<T>::ErrLoanAccrueFailed)?;

				loan_data.accumulated_rate = accumulated_rate;
				loan_data.last_updated = now;
				loan_data.write_off_index = Some(write_off_group_index);

				// calculate updated write off adjusted present value
				let new_pv = loan_data
					.present_value_with_write_off(write_off_groups)
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

				// update nav
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;

				// update loan data
				*maybe_loan_data = Some(loan_data);
				Ok(write_off_group_index)
			},
		)
	}
}

impl<T: Config> TPoolNav<T::PoolId, T::Amount> for Pallet<T> {
	fn nav(pool_id: T::PoolId) -> Option<(T::Amount, u64)> {
		PoolNAV::<T>::get(pool_id)
			.and_then(|nav_details| Some((nav_details.latest_nav, nav_details.last_updated)))
	}

	fn update_nav(pool_id: T::PoolId) -> Result<T::Amount, DispatchError> {
		Self::update_nav_of_pool(pool_id)
	}
}

/// Ensure origin that allows only loan pallet account
pub struct EnsureLoanAccount<T>(sp_std::marker::PhantomData<T>);

impl<
		T: pallet::Config,
		Origin: Into<Result<RawOrigin<T::AccountId>, Origin>> + From<RawOrigin<T::AccountId>>,
	> EnsureOrigin<Origin> for EnsureLoanAccount<T>
{
	type Success = T::AccountId;

	fn try_origin(o: Origin) -> Result<Self::Success, Origin> {
		let loan_id = T::LoanPalletId::get().into_account();
		o.into().and_then(|o| match o {
			RawOrigin::Signed(who) if who == loan_id => Ok(loan_id),
			r => Err(Origin::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> Origin {
		let loan_id = T::LoanPalletId::get().into_account();
		Origin::from(RawOrigin::Signed(loan_id))
	}
}
