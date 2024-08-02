use cfg_types::domain_address::Domain;
use frame_support::{
	pallet_prelude::ValueQuery,
	storage::{child::MultiRemovalResults, generator::StorageValue, unhashed},
	storage_alias,
	traits::{Get, OnRuntimeUpgrade},
	Blake2_128Concat, StorageMap,
};
use pallet_liquidity_pools_gateway::{pallet::Pallet as LPGateway, Config};
use pallet_order_book::weights::Weight;
use sp_runtime::{DispatchError, TryRuntimeError};
use sp_std::vec::Vec;

#[storage_alias]
pub type OutboundMessageNonceStore<T: Config> =
	StorageValue<pallet_liquidity_pools_gateway::Pallet<T>, u64, ValueQuery>;

#[storage_alias]
pub type OutboundMessageQueue<T: Config + frame_system::Config> = StorageMap<
	pallet_liquidity_pools_gateway::Pallet<T>,
	Blake2_128Concat,
	u64,
	(
		Domain,
		<T as frame_system::Config>::AccountId,
		pallet_liquidity_pools::Message,
	),
>;

#[storage_alias]
pub type FailedOutboundMessages<T: Config + frame_system::Config> = StorageMap<
	pallet_liquidity_pools_gateway::Pallet<T>,
	Blake2_128Concat,
	u64,
	(
		Domain,
		<T as frame_system::Config>::AccountId,
		pallet_liquidity_pools::Message,
		DispatchError,
	),
>;

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: Config + frame_system::Config,
{
	fn on_runtime_upgrade() -> Weight {
		let mut weight = Self::clear_storage(
			OutboundMessageNonceStore::<T>::storage_prefix(),
			"OutboundMessageNonceStore",
		);

		for (nonce, entry) in OutboundMessageQueue::<T>::iter() {
			log::warn!("Found outbound message:\nnonce:{nonce}\nentry:{entry:?}");
			weight = weight.saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));
		}

		for (nonce, entry) in FailedOutboundMessages::<T>::iter() {
			log::warn!("Found failed outbound message:\nnonce:{nonce}\nentry:{entry:?}");
			weight = weight.saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));
		}

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
		// Extra check to confirm that the storage alias is correct.
		assert!(
			OutboundMessageNonceStore::<T>::get() > 0,
			"OutboundMessageNonce should be > 0"
		);

		assert_eq!(
			OutboundMessageQueue::<T>::iter_keys().count(),
			0,
			"OutboundMessageQueue should be empty!"
		);

		assert_eq!(
			FailedOutboundMessages::<T>::iter_keys().count(),
			0,
			"FailedOutboundMessages should be empty!"
		);

		Ok(Vec::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), TryRuntimeError> {
		assert_eq!(
			OutboundMessageNonceStore::<T>::get(),
			0,
			"OutboundMessageNonceStore should be 0!"
		);

		assert_eq!(
			OutboundMessageQueue::<T>::iter_keys().count(),
			0,
			"OutboundMessageQueue should be empty!"
		);

		assert_eq!(
			FailedOutboundMessages::<T>::iter_keys().count(),
			0,
			"FailedOutboundMessages should be empty!"
		);

		Ok(())
	}
}

impl<T: Config + frame_system::Config> Migration<T> {
	fn clear_storage(prefix: &[u8], storage_name: &str) -> Weight {
		let res = unhashed::clear_prefix(prefix, None, None);

		match res.maybe_cursor {
			None => log::info!("{storage_name} was cleared"),
			Some(_) => log::error!("{storage_name} was not completely cleared"),
		};

		<T as frame_system::Config>::DbWeight::get()
			.reads_writes(res.loops.into(), res.unique.into())
	}
}
