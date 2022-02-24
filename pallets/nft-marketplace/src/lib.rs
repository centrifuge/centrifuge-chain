//! NFT Marketplace pallet
//!
//! This pallet provides a marketplace for digital art creators and owners to enlist their
//! NFTs for sale and for potential buyers to browse and buy NFTs.
//!
//! To set an NFT for sale, users will call `add`, which will add the NFT to the gallery
//! of NFT that open for sale. Doing so will have the NFT being transferred from the seller
//! to this pallet's account.
//!
//! To remove an NFT from sale and thus reclaim its ownership, sellers can call `remove`.
//!
//! To buy an NFT, any account besides the seller of an NFT being purchased can call `buy`.
//!
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	traits::{
		fungibles::{self, Transfer as FungiblesTransfer},
		tokens::nonfungibles::{
			self, Inspect as _, Transfer as NonFungiblesTransfer,
		},
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

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

type CurrencyOf<T> =
	<<T as pallet::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::AssetId;

type BalanceOf<T> =
	<<T as pallet::Config>::Fungibles as fungibles::Inspect<AccountIdOf<T>>>::Balance;

type SaleOf<T> = Sale<AccountIdOf<T>, CurrencyOf<T>, BalanceOf<T>>;

/// type alias to Non fungible ClassId type
type ClassIdOf<T> = <<T as Config>::NonFungibles as nonfungibles::Inspect<AccountIdOf<T>>>::ClassId;

/// type alias to Non fungible InstanceId type
type InstanceIdOf<T> =
	<<T as Config>::NonFungibles as nonfungibles::Inspect<AccountIdOf<T>>>::InstanceId;

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
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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

		/// An operation expected an NFT to be for sale when it is not
		NotForSale,

		/// A buyer attempted to buy an NFT they are selling
		IsSeller,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add the given NFT to the gallery, putting it for sale.
		///
		/// Fails if
		///   - the NFT is not found in [T::NonFungibles]
		///   - `origin` is not the owner of the nft
		///   - the nft is already for sale in the gallery
		///   - transferring ownership of the NFT to this pallet's account fails
		#[pallet::weight(10_000_000)]
		pub fn add(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
			currency: CurrencyOf<T>,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let seller = ensure_signed(origin.clone())?;

			// Check that the seller is the owner of the nft
			ensure!(
				Self::is_owner(seller.clone(), class_id, instance_id)?,
				Error::<T>::NotOwner,
			);

			// Check that the nft is not yet for sale
			ensure!(
				!<Gallery<T>>::contains_key(class_id, instance_id),
				Error::<T>::AlreadyForSale
			);

			// Transfer the NFT to the parachain account
			T::NonFungibles::transfer(&class_id.into(), &instance_id.into(), &Self::account())?;

			// Put the nft for sale
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
		/// The sellers of an NFT that is for sale can call this extrinsic to reclaim
		/// ownership over their NFT and remove it from sale.
		///
		/// Fails if
		///   - the nft is not for sale
		///   - `origin` is not the seller of the NFT
		///   - transferring the ownership of the NFT back to the seller fails
		#[pallet::weight(10_000_000)]
		pub fn remove(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			let sale = <Gallery<T>>::get(class_id, instance_id).ok_or(Error::<T>::NotForSale)?;

			// Ensure that the buyer is not the seller of the NFT
			ensure!(who == sale.seller, Error::<T>::NotOwner);

			// Transfer the NFT back to the seller, i.e., the original owner of this NFT
			T::NonFungibles::transfer(&class_id.into(), &instance_id.into(), &sale.seller)?;

			// Remove the NFT from the gallery
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
		///   - the NFT is not for sale
		///   - `origin` is the seller of the NFT
		///   - `origin` does not have enough balance of the currency the nft is being sold in
		///   - transferring the asking price from the buyer to the seller fails
		///   - transferring the nft to the buyer fails
		#[pallet::weight(10_000_000)]
		#[transactional]
		pub fn buy(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			instance_id: T::InstanceId,
		) -> DispatchResult {
			let buyer = ensure_signed(origin.clone())?;
			let sale = <Gallery<T>>::get(class_id, instance_id).ok_or(Error::<T>::NotForSale)?;

			// Ensure that the buyer is not the seller of the NFT
			ensure!(buyer != sale.seller, Error::<T>::IsSeller);

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
			T::NonFungibles::owner(&class_id.into(), &instance_id.into())
				.map(|owner| owner == account)
				.ok_or(Error::<T>::NotFound)
		}

		pub fn account() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		pub fn origin() -> T::Origin {
			RawOrigin::from(Some(Self::account())).into()
		}
	}
}
