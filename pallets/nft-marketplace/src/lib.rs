//! NFT Marketplace pallet
//!
//! This pallet provides a marketplace for digital art creators and owners
//! to enlist their NFTs for sale and for potential buyers to browse and
//! buy NFTs.
//!
//! // TODO(nuno): Explain more, including how the NFTs will be locked once set for sale.
//!
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	traits::fungibles::{self, Inspect, Transfer},
};
use frame_system::ensure_signed;
use scale_info::TypeInfo;
use sp_runtime::traits::{AccountIdConversion, StaticLookup};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

type CurrencyOf<T> =
	<<T as pallet::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::AssetId;

type BalanceOf<T> =
	<<T as pallet::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::Balance;

type SaleOf<T> = Sale<AccountIdOf<T>, CurrencyOf<T>, BalanceOf<T>>;

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Sale<AccountId, CurrencyId, Balance> {
	pub seller: AccountId,
	pub price: AskingPrice<CurrencyId, Balance>,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct AskingPrice<CurrencyId, Balance> {
	pub currency: CurrencyId,
	pub amount: Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::{transactional, PalletId};
	use frame_system::pallet_prelude::*;
	use frame_system::RawOrigin;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_uniques::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Fungibles implements fungibles::Transfer, granting us a way of charging
		/// the buyer of an NFT the respective asking price.
		type Fungibles: fungibles::Transfer<Self::AccountId>;

		/// PalletID of this loan module
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	/// The gallery of NFTs currently for sale
	#[pallet::storage]
	#[pallet::getter(fn get_allowlisted)]
	pub(super) type Gallery<T: Config> = StorageDoubleMap<
		_,
		// The hasher for the first key
		Blake2_128Concat,
		// The first key, the nft class Id
		T::ClassId,
		// The hasher for the second key
		Blake2_128Concat,
		// The second key, the nft instance id
		T::InstanceId,
		// The data regarding this item open for sale
		SaleOf<T>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An NFT has been added to the gallery and is now for sale
		ForSale(SaleOf<T>),

		/// An NFT was removed from the gallery and is no longer for sale
		Removed {
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		},

		/// An NFT has been sold
		Sold {
			sale: SaleOf<T>,
			buyer: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A user tried to add an NFT that could not be found
		NotFound,

		/// The origin is not the owner of an NFT
		NotOwner,

		/// A seller has attempted to list an NFT that is already for sale
		AlreadyForSale,

		/// A buyer attempted to buy an NFT that is not for sale
		NotForSale,

		/// A buyer attempted to buy an NFT they already own
		AlreadyOwner,

		/// This pallet was not given enough permission (Freezer + Admin) to manage an asset
		NoPermission,

		/// The seller does not have sufficient balance to buy the asset
		InsufficientBalance,

		/// Payment failed, i.e, failed to transfer the asking price from the buyer to the seller
		PaymentFailed,

		/// Failed to transfer a purchased NFT
		FailedNftTransfer,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add the given NFT to the gallery, putting it for sale.
		///
		/// Fails if
		///   - the NFT is not found in `pallet_uniques`
		///   - `origin` is not the owner of the nft
		///   - this pallet has not been set to be the freezer of the asset
		///   - the nft is already for sale in the gallery
		#[pallet::weight(10_000_000)]
		pub fn add(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
			currency: CurrencyOf<T>,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let seller = ensure_signed(origin.clone())?;

			// Check that the seller is the owner of the asset
			ensure!(
				Self::is_owner(seller.clone(), class_id, instance_id)?,
				Error::<T>::NotOwner,
			);

			// Check that the asset is not yet for sale
			ensure!(
				!<Gallery<T>>::contains_key(class_id, instance_id),
				Error::<T>::AlreadyForSale
			);

			// Freeze the asset to disallow unprivileged transfers
			<pallet_uniques::Pallet<T>>::freeze(Self::origin(), class_id, instance_id)
				.map_err(|_| Error::<T>::NoPermission)?;

			// Put the asset for sale
			let sale = Sale {
				seller,
				price: AskingPrice { currency, amount },
			};
			<Gallery<T>>::insert(class_id, instance_id, sale.clone());
			Self::deposit_event(Event::ForSale(sale));

			Ok(())
		}

		/// Remove an NFT from sale
		///
		/// Fails if
		///   - `origin` is not the owner of the NFT
		///   - the nft is not for sale
		#[pallet::weight(10_000_000)]
		pub fn remove(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Check that origin is the owner of the asset
			ensure!(
				Self::is_owner(who, class_id, instance_id)?,
				Error::<T>::NotOwner,
			);

			// Check that this NFT is for sale
			ensure!(
				<Gallery<T>>::contains_key(class_id, instance_id),
				Error::<T>::NotForSale
			);

			// Try and thaw the asset, fails if this pallet is not its freezer anymore but we don't
			// need to do anything about that.
			let _ = <pallet_uniques::Pallet<T>>::thaw(Self::origin(), class_id, instance_id);

			<Gallery<T>>::remove(class_id, instance_id);
			Self::deposit_event(Event::Removed {
				class_id,
				instance_id,
			});

			Ok(())
		}

		/// Buy the given nft
		///
		/// Fails if
		///   - `origin` does not have enough balance of the currency the nft is being sold in
		///   - the specified NFT does not exist in the gallery
		///   - this pallet is not an admin of the NFT class and can't therefore transfer ownership
		///   - transferring the nft from the seller to the buyer fails
		///   - transferring the asking price fails
		#[pallet::weight(10_000_000)]
		#[transactional]
		pub fn buy(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		) -> DispatchResult {
			let buyer = ensure_signed(origin.clone())?;

			// Ensure that the buyer is not the owner of the asset already
			ensure!(
				!Self::is_owner(buyer.clone(), class_id, instance_id)?,
				Error::<T>::AlreadyOwner,
			);

			let sale = <Gallery<T>>::get(class_id, instance_id).ok_or(Error::<T>::NotForSale)?;

			// Make sure the buyer can pay for the NFT
			T::Fungibles::can_withdraw(sale.price.currency, &buyer, sale.price.amount)
				.into_result()
				.map_err(|_| Error::<T>::InsufficientBalance)?;

			// Have the buyer pay for the NFT
			T::Fungibles::transfer(
				sale.price.currency,
				&buyer,
				&sale.seller,
				sale.price.amount,
				true,
			)
			.map_err(|_| Error::<T>::PaymentFailed)?;

			// Thaw the NFT so that we can transfer it
			<pallet_uniques::Pallet<T>>::thaw(Self::origin(), class_id, instance_id)
				.map_err(|_| Error::<T>::NoPermission)?;

			// Transfer the NFT to the buyer
			let buyer_lookup = T::Lookup::unlookup(buyer.clone());
			<pallet_uniques::Pallet<T>>::transfer(
				Self::origin(),
				class_id,
				instance_id,
				buyer_lookup,
			)
			.map_err(|_| Error::<T>::FailedNftTransfer)?;

			<Gallery<T>>::remove(class_id, instance_id);
			Self::deposit_event(Event::Sold { sale, buyer });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn is_owner(
			account: T::AccountId,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		) -> Result<bool, Error<T>> {
			<pallet_uniques::Pallet<T>>::owner(class_id, instance_id)
				.map(|owner| owner == account)
				.ok_or(Error::<T>::NotFound)
		}

		#[allow(dead_code)]
		pub fn account() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		pub fn origin() -> T::Origin {
			RawOrigin::from(Some(Self::account())).into()
		}
	}
}
