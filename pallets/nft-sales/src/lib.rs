//! NFT Sales pallet
//!
//! This pallet provides a place for digital art creators and owners to offer their
//! NFTs for sale and for potential buyers to browse and buy them.
//!
//! To sell NFTs, users will call `add`. Doing so will have the NFT being transferred
//! from the seller to this pallet's account, to grant us the own
//!
//! To remove an NFT from sale and thus reclaim its ownership, sellers will call `remove`.
//!
//! To buy an NFT, users will call `buy`.
//!
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	traits::{
		fungibles::{self, Transfer as FungiblesTransfer},
		tokens::nonfungibles::{self, Inspect as _, Transfer as _},
	},
};
use frame_system::ensure_signed;
use scale_info::TypeInfo;
use sp_runtime::traits::AccountIdConversion;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

// Type aliases
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

type CurrencyOf<T> =
	<<T as pallet::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::AssetId;

type BalanceOf<T> =
	<<T as pallet::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::Balance;

type SaleOf<T> = Sale<AccountIdOf<T>, CurrencyOf<T>, BalanceOf<T>>;

type ClassIdOf<T> = <<T as Config>::NonFungibles as nonfungibles::Inspect<AccountIdOf<T>>>::ClassId;

type InstanceIdOf<T> =
	<<T as Config>::NonFungibles as nonfungibles::Inspect<AccountIdOf<T>>>::InstanceId;

// Storage types
#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Sale<AccountId, CurrencyId, Balance> {
	pub seller: AccountId,
	pub price: Price<CurrencyId, Balance>,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Price<CurrencyId, Balance> {
	pub currency: CurrencyId,
	pub amount: Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::{transactional, PalletId};
	use frame_system::pallet_prelude::*;
	use frame_system::RawOrigin;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type WeightInfo: WeightInfo;

		/// Fungibles implements fungibles::Transfer, granting us a way of charging
		/// the buyer of an NFT the respective asking price.
		type Fungibles: fungibles::Transfer<Self::AccountId>;

		/// The NonFungibles trait impl that can transfer and inspect NFTs.
		type NonFungibles: nonfungibles::Transfer<Self::AccountId>;

		/// The NFT ClassId type
		type ClassId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ Default
			+ TypeInfo
			+ IsType<ClassIdOf<Self>>;

		/// The NFT InstanceId type
		type InstanceId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ TypeInfo
			+ From<u128>
			+ IsType<InstanceIdOf<Self>>;

		/// The Id of this pallet
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	/// The active sales.
	/// A sale is an entry identified by an NFT class and instance id.
	#[pallet::storage]
	#[pallet::getter(fn get_allowlisted)]
	pub(super) type Sales<T: Config> = StorageDoubleMap<
		_,
		// The hasher for the first key
		Blake2_128Concat,
		// The first key, the nft class Id
		T::ClassId,
		// The hasher for the second key
		Blake2_128Concat,
		// The second key, the nft instance id
		T::InstanceId,
		// The data regarding the sale
		SaleOf<T>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An NFT is now for sale
		ForSale {
			class_id: T::ClassId,
			instance_id: T::InstanceId,
			sale: SaleOf<T>,
		},

		/// An NFT was removed
		Removed {
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		},

		/// An NFT has been sold
		Sold {
			class_id: T::ClassId,
			instance_id: T::InstanceId,
			sale: SaleOf<T>,
			buyer: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A user tried to add an NFT that could not be found in T::NonFungibles
		NotFound,

		/// The origin is not the owner of an NFT
		NotOwner,

		/// A seller has attempted to add an NFT that is already for sale
		AlreadyForSale,

		/// An operation expected an NFT to be for sale when it is not
		NotForSale,

		/// A buyer's max offer is invalid, i.e., either the currency or amount did
		/// not match the latest asking price for the targeted NFT.
		InvalidOffer,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add an NFT
		///
		/// Fails if
		///   - the NFT is not found in [T::NonFungibles]
		///   - `origin` is not the owner of the nft
		///   - the nft is already for sale
		///   - transferring ownership of the NFT to this pallet's account fails
		#[pallet::weight(<T as Config>::WeightInfo::add())]
		pub fn add(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
			price: Price<CurrencyOf<T>, BalanceOf<T>>,
		) -> DispatchResult {
			let seller = ensure_signed(origin.clone())?;

			// Check that the seller is the owner of the nft
			ensure!(
				Self::is_owner(seller.clone(), class_id, instance_id)?,
				Error::<T>::NotOwner,
			);

			// Ensure that the nft is not for sale
			ensure!(
				!<Sales<T>>::contains_key(class_id, instance_id),
				Error::<T>::AlreadyForSale
			);

			// Transfer the NFT to the parachain account
			T::NonFungibles::transfer(&class_id.into(), &instance_id.into(), &Self::account())?;

			// Put the nft for sale
			let sale = Sale { seller, price };
			<Sales<T>>::insert(class_id, instance_id, sale.clone());
			Self::deposit_event(Event::ForSale {
				class_id,
				instance_id,
				sale,
			});

			Ok(())
		}

		/// Remove an NFT
		///
		/// The seller of an NFT that is for sale can call this extrinsic to reclaim
		/// ownership over their NFT and remove it from sale.
		///
		/// Fails if
		///   - the nft is not for sale
		///   - `origin` is not the seller of the NFT
		///   - transferring the ownership of the NFT back to the seller fails
		#[pallet::weight(<T as Config>::WeightInfo::remove())]
		pub fn remove(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			let sale = <Sales<T>>::get(class_id, instance_id).ok_or(Error::<T>::NotForSale)?;

			// Ensure that the buyer is not the seller of the NFT
			ensure!(who == sale.seller, Error::<T>::NotOwner);

			// Transfer the NFT back to the seller, i.e., the original owner of this NFT
			T::NonFungibles::transfer(&class_id.into(), &instance_id.into(), &sale.seller)?;

			// Remove the NFT from the active sales
			<Sales<T>>::remove(class_id, instance_id);
			Self::deposit_event(Event::Removed {
				class_id,
				instance_id,
			});

			Ok(())
		}

		/// Buy the given nft
		///
		/// Buyers must propose a `max_offer` to save them from a scenario where they could end up
		/// paying more than they desired for an NFT. That scenario could take place if the seller
		/// increased the asking price right before the buyer submits this call to buy said NFT.
		///
		/// Buyers always pay the latest asking price as long as it does not exceed their max offer.
		///
		/// Fails if
		///   - the NFT is not for sale
		///   - `origin` is the seller of the NFT
		///   - `origin` does not have enough balance of the currency the nft is being sold in
		///   - transferring the asking price from the buyer to the seller fails
		///   - transferring the nft to the buyer fails
		#[pallet::weight(<T as Config>::WeightInfo::buy())]
		#[transactional]
		pub fn buy(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
			max_offer: Price<CurrencyOf<T>, BalanceOf<T>>,
		) -> DispatchResult {
			let buyer = ensure_signed(origin.clone())?;
			let sale = <Sales<T>>::get(class_id, instance_id).ok_or(Error::<T>::NotForSale)?;

			ensure!(
				sale.price.currency == max_offer.currency && sale.price.amount <= max_offer.amount,
				Error::<T>::InvalidOffer,
			);

			// Have the buyer pay the seller for the NFT
			T::Fungibles::transfer(
				sale.price.currency,
				&buyer,
				&sale.seller,
				sale.price.amount,
				true,
			)?;

			// Transfer the NFT to the buyer
			T::NonFungibles::transfer(
				// Self::origin(),
				&class_id.into(),
				&instance_id.into(),
				&buyer.clone(),
			)?;

			<Sales<T>>::remove(class_id, instance_id);
			Self::deposit_event(Event::Sold {
				class_id,
				instance_id,
				sale,
				buyer,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Check if the given `account` is the owner of the NFT.
		/// Returns:
		///		- Ok(bool) when the NFT is found in T::NonFungibles
		///     - Err(NotFound) when the NFT could not be found in T::NonFungibles
		fn is_owner(
			account: T::AccountId,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		) -> Result<bool, Error<T>> {
			T::NonFungibles::owner(&class_id.into(), &instance_id.into())
				.map(|owner| owner == account)
				.ok_or(Error::<T>::NotFound)
		}

		pub fn origin() -> T::Origin {
			RawOrigin::from(Some(Self::account())).into()
		}

		pub fn account() -> T::AccountId {
			T::PalletId::get().into_account()
		}
	}
}
