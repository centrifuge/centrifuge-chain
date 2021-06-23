//! # Fees pallet for runtime
//!
//! This pallet provides functionality for setting and getting fees associated with an Hash key..
//! Fees are set by FeeOrigin or RootOrigin
#![cfg_attr(not(feature = "std"), no_std)]
use frame_support::weights::Weight;

pub use pallet::*;
pub mod weights;
use crate::traits::RuntimeUpgradeProvider;
use frame_support::sp_runtime::traits::Zero;
pub use weights::*;

pub mod traits {
	use codec::FullCodec;
	use frame_support::dispatch::Weight;
	use sp_version::RuntimeVersion;

	pub type UsableWeight = Weight;
	pub type UsedWeight = Weight;

	// pub type Upgrader = Box<dyn FnOnce(UsableWeight) -> (Finished, UsedWeight)>;

	pub trait RuntimeUpgradeProvider {
		/// A memo to a specific upgrade. In most cases this should briefly! describe
		/// what was done in this upgrade.
		type Memo: Clone + Eq + sp_std::fmt::Debug + AsRef<[u8]>;

		/// An info that needs to be defined by the implementer of this trait.
		///
		/// The calley can uses this info in order to memorize himself, which part of the
		/// upgrade needs to be run next. The lastly received `StateInfo` will always be provided
		/// to the implementer upon call of `next`.
		///
		/// E.g. One might uses this info in order to sequence a runtime upgrade or in order to
		/// split a large upgrade of one pallet into multiple steps. So that each step fits into a
		/// block.
		type StateInfo: sp_std::default::Default + FullCodec;

		/// General runtime upgrade info.
		/// The `RuntimeVersion` and the `Memo` are stored under the block number the upgrade was
		/// started.
		fn info() -> (RuntimeVersion, Self::Memo);

		/// The accumulated weight of the runtime upgrade. This value is used to compute the
		/// number of blocks, that this upgrade will block the chain.
		fn upgrade_weight() -> Weight;

		/// The runtime calls the next part of the runtime to be run. Hereby, the runtime
		/// indicates how much weight, the part must at most use.
		///
		/// It is up to the calley to ensure, that `usable` is not exceeded by the upgrade!
		fn next(
			usable: UsableWeight,
			last_state: Option<Self::StateInfo>,
		) -> (UsedWeight, Self::StateInfo);
	}
}

#[frame_support::pallet]
pub mod pallet {

	// Import various types used to declare pallet in scope.
	use super::*;
	use crate::traits::RuntimeUpgradeProvider;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::{Convert, Saturating};
	use frame_support::sp_runtime::traits::{One, Zero};
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;
	use sp_version::RuntimeVersion;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Upgrader structure that provides the actual logic for the upgrades
		type Upgrades: RuntimeUpgradeProvider;

		/// Used in order to convert the given weights into a number of blocks
		type WeightToBlockNumber: Convert<Weight, Self::BlockNumber>;

		/// Associated type for Event enum
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_runtime_upgrade() -> Weight {
			// TODO: Insert some mechanism to prevent the accidential execution of the same
			// 	upgreade twice. Probably best, to let the runtime version be the key for the
			// 	map of old upgrades and if it already exists, do not do the upgrade!
			let max_per_block = T::BlockWeights::get().max_block / 2;

			// In order to safely unwrap below, we do this check. Although, one might argue
			// a chain with a maximum weight of zero, might be useless.
			if max_per_block == <Weight as Zero>::zero() {
				// As we are not updating the storage, this will basically prevent the upgrade from
				// running.
				return Zero::zero();
			}

			// The number of blocks we are going to block the chain for the upgreade
			//
			// NOTE:
			// For the reasons of missing a remainder we are adding 1 extra block at the end,
			// so that the upgreade will definitely fit into the given blocks.
			let period: T::BlockNumber = <T::WeightToBlockNumber>::convert(
				T::Upgrades::upgrade_weight()
					.checked_div(max_per_block)
					.expect("Maximum block weight is not zero. qed."),
			)
			.saturating_add(One::one());

			let now = <frame_system::Pallet<T>>::block_number();
			// Insert start and period into storage for the `on_initialize`
			<UpgradeStart<T>>::set(now);
			<UpgradePeriod<T>>::set(period);

			let (version, memo) = T::Upgrades::info();
			<UpgradeInfo<T>>::insert(now, (version.clone(), Vec::from(memo.as_ref())));

			Self::deposit_event(Event::RuntimeUpgradeStarted(period, version));

			// return zero as `on_initialize` will take care of the upgrade
			Zero::zero()
		}

		fn on_initialize(n: T::BlockNumber) -> frame_support::weights::Weight {
			// You can not do an upgrade on the first block...
			if n < Self::upgrade_start().saturating_add(Self::upgrade_period()) && n != Zero::zero()
			{
				let period = Self::upgrade_period();
				let start = Self::upgrade_start();
				let max_per_block = T::BlockWeights::get().max_block / 2;

				Self::upgrade(max_per_block);

				// Indicate to the network, that the upgrade has been executed successfully
				if (n == start.saturating_add(period) - One::one())
					&& <UpgradeInfo<T>>::contains_key(start)
				{
					let (version, memo) = <UpgradeInfo<T>>::get(start)
						.expect("The storage is populated with the current upgread. qed.");

					// Remove the last StateInfo from storage
					<UpgradeState<T>>::set(None);

					Self::deposit_event(Event::RuntimeUpgradeFinished(version, memo));
				}

				max_per_block
			} else {
				Zero::zero()
			}
		}
	}

	#[pallet::type_value]
	pub fn OnUpgradeStartEmpty<T: Config>() -> T::BlockNumber {
		Zero::zero()
	}

	#[pallet::storage]
	#[pallet::getter(fn upgrade_start)]
	/// The start block of a runtime upgrade
	pub(super) type UpgradeStart<T: Config> =
		StorageValue<_, T::BlockNumber, ValueQuery, OnUpgradeStartEmpty<T>>;

	#[pallet::type_value]
	pub fn OnUpgradePeriodEmpty<T: Config>() -> T::BlockNumber {
		Zero::zero()
	}

	#[pallet::storage]
	#[pallet::getter(fn upgrade_period)]
	/// The start block of a runtime upgrade
	pub(super) type UpgradePeriod<T: Config> =
		StorageValue<_, T::BlockNumber, ValueQuery, OnUpgradePeriodEmpty<T>>;

	#[pallet::storage]
	#[pallet::getter(fn upgrade_info)]
	/// The start block of a runtime upgrade
	pub(super) type UpgradeInfo<T: Config> =
		StorageMap<_, Twox64Concat, T::BlockNumber, (RuntimeVersion, Vec<u8>), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn upgrade_state)]
	/// The start block of a runtime upgrade
	pub(super) type UpgradeState<T: Config> = StorageValue<
		_,
		<<T as pallet::Config>::Upgrades as RuntimeUpgradeProvider>::StateInfo,
		OptionQuery,
	>;

	/// Pallet genesis configuration type declaration.
	///
	/// It allows to build genesis storage.
	#[pallet::genesis_config]
	pub struct GenesisConfig {}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self {}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Indicates that a runtime upgrade will happen.
		/// [Number of blocks the runtime will need, runtime versioned that is upgraded to]
		RuntimeUpgradeStarted(T::BlockNumber, RuntimeVersion),

		/// Indicates that a runtime upgreade happened succesfully
		RuntimeUpgradeFinished(RuntimeVersion, Vec<u8>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Upgrade can not be splitted in a way, that one portion of it fits into a single block
		UpgradeToHeavy,
	}
}

impl<T: Config> Pallet<T> {
	fn upgrade(mut available: Weight) {
		let mut info = Self::upgrade_state();

		while available >= Zero::zero() {
			let (used, next_info) = T::Upgrades::next(available, info);

			available = available.saturating_sub(used);
			info = Some(next_info);
		}

		<UpgradeState<T>>::set(info);
	}
}
