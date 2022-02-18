//! NFT Marketplace pallet
//!
//! This pallet provides a marketplace for digital art creators and owners
//! to enlist their NFTs for sale and for potential buyers to browse and
//! buy NFTs.
//!
//! // TODO(nuno): Explain more, including how the NFTs will be locked once set for sale.
//!
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::DispatchResult;
use frame_system::ensure_signed;
use sp_runtime::traits::AtLeast32BitUnsigned;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The NFT class id
		type ClassId: Parameter + Member + MaybeSerializeDeserialize + Copy + Default + TypeInfo;

		/// The NFT instance id
		type InstanceId: Parameter + Member + MaybeSerializeDeserialize + Copy + Default + TypeInfo;

		/// The supported currencies that NFTs can be sold in
		type CurrencyId: Parameter + Member + MaybeSerializeDeserialize + Copy + Default + TypeInfo;

		/// The type for the asking price amount
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		//TODO(nuno): we also need an impl of fungibles to move balances when having the buyer
		// paying the asking price
	}

	// The genesis config type.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		//TODO(nuno): define this type appropriately later
		pub initial_state: Option<T::Balance>,
	}

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				initial_state: None,
			}
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			//TODO(nuno): prefill the gallery with the initial state once that's ready.
		}
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
		// The asking price
		(T::CurrencyId, T::Balance),
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	//TODO(nuno): given these meaningful params
	pub enum Event<T: Config> {
		/// An NFT has been added to the gallery and is now for sale
		ForSale,

		/// An NFT has been removed from the gallery and is no longer for sale
		Unlisted,

		/// An NFT has been sold
		Sold,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The origin is not the owner of an NFT
		NotOwner,

		/// A seller has attempted to list an NFT that is already for sale
		AlreadyForSale,

		/// A buyer attempted to buy an NFT that is not for sale
		NotForSale,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Put the given NFT for sale
		/// Fails if
		///   - `origin` is not the owner of the nft
		///   - TODO(nuno)
		#[pallet::weight(10_000_000)]
		pub fn add(origin: OriginFor<T>) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			// TODO(nuno): implement this
			Ok(())
		}

		/// Remove an nft from sale
		///
		/// Fails if
		///   - `origin` is not the owner of the NFT
		///   - the nft is not for sale
		#[pallet::weight(10_000_000)]
		pub fn remove(origin: OriginFor<T>) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			// TODO(nuno): implement this
			Ok(())
		}

		/// Buy the given nft
		///
		/// Fails if
		///   - `origin` does not have enough balance of the currency the nft is being sold in
		///   - the specified NFT does not exist in the gallery
		///   - this pallet is not an admin of the NFT class and can't therefore transfer ownership
		///   - transferring the asking price fails
		#[pallet::weight(10_000_000)]
		pub fn buy(origin: OriginFor<T>) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			// TODO(nuno): implement this
			Ok(())
		}
	}
}
