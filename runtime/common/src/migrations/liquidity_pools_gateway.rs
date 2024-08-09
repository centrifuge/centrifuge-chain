use cfg_types::domain_address::Domain;
use frame_support::{
	pallet_prelude::ValueQuery,
	storage_alias,
	traits::{Get, OnRuntimeUpgrade},
	Blake2_128Concat,
};
use pallet_order_book::weights::Weight;
use sp_runtime::DispatchError;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

#[storage_alias]
pub type OutboundMessageNonceStore<T: pallet_liquidity_pools_gateway::Config> =
	StorageValue<pallet_liquidity_pools_gateway::Pallet<T>, u64, ValueQuery>;

#[storage_alias]
pub type OutboundMessageQueue<T: pallet_liquidity_pools_gateway::Config + frame_system::Config> =
	StorageMap<
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
pub type FailedOutboundMessages<T: pallet_liquidity_pools_gateway::Config + frame_system::Config> =
	StorageMap<
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

const LOG_PREFIX: &str = "LiquidityPoolsGatewayV1";

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: pallet_liquidity_pools_gateway::Config + frame_system::Config,
{
	fn on_runtime_upgrade() -> Weight {
		let mut weight = Weight::zero();
		OutboundMessageNonceStore::<T>::kill();

		for (nonce, entry) in OutboundMessageQueue::<T>::iter() {
			log::warn!("{LOG_PREFIX}: Found outbound message:\nnonce:{nonce}\nentry:{entry:?}");
			weight.saturating_accrue(T::DbWeight::get().reads(1));
		}

		for (nonce, entry) in FailedOutboundMessages::<T>::iter() {
			log::warn!(
				"{LOG_PREFIX}: Found failed outbound message:\nnonce:{nonce}\nentry:{entry:?}"
			);
			weight.saturating_accrue(T::DbWeight::get().reads(1));
		}

		log::info!("{LOG_PREFIX}: Migration done!");

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		// Extra check to confirm that the storage alias is correct.
		assert_eq!(
			OutboundMessageQueue::<T>::iter_keys().count(),
			0,
			"{LOG_PREFIX}: OutboundMessageQueue should be empty!"
		);

		assert_eq!(
			FailedOutboundMessages::<T>::iter_keys().count(),
			0,
			"{LOG_PREFIX}: FailedOutboundMessages should be empty!"
		);

		log::info!("{LOG_PREFIX}: Pre checks done!");

		Ok(Vec::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert_eq!(
			OutboundMessageNonceStore::<T>::get(),
			0,
			"{LOG_PREFIX}: OutboundMessageNonceStore should be 0!"
		);

		assert_eq!(
			OutboundMessageQueue::<T>::iter_keys().count(),
			0,
			"{LOG_PREFIX}: OutboundMessageQueue should be empty!"
		);

		assert_eq!(
			FailedOutboundMessages::<T>::iter_keys().count(),
			0,
			"{LOG_PREFIX}: FailedOutboundMessages should be empty!"
		);

		log::info!("{LOG_PREFIX}: Post checks done!");

		Ok(())
	}
}
