
// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.


//! # Crowdloan reward pallet
//!
//! ## Overview
//! The function of this pallet is to provide the Centrifuge-specific reward functionality for
//! contributors of the relay chain crowdloan. In order to provide this functionality the pallet
//! implements the `Reward` Trait from the `Claim` Pallet.
//! Before any rewards can be provided to contributors the pallet MUST be initialized, so that the
//! modules account holds the necessary funds to reward.
//! All rewards are payed in form of a `vested_transfer` with a fixed vesting time. The vesting time
//! is defined via the modules `Config` trait.
//!
//! ## Callable Functions
//!
//! - [`set_vesting_start`] : Origin must be admin or root, and allows to set the start of the vesting
//!    after the initialization.
//! - [`set_vesting_period`] : Origin must be admin or root, and allows to set the period of the vesting
//!    after the initialization.
//! - [`set_conversion_rate`] : Origin must be admin or root, and allows to set the conversion rate
//!    between relay chain and native balance after the initialization.
//! - [`set_direct_payout_ratio`] : Origin must be admin or root, and allows to set the ratio between
//!    vested and direct payout amount after the initialization.
//!
//! ## Implementations
//!
//! - [`Reward`](crowdloan_claim::reward::Reward) : Rewarding functionality for contributors in a relay
//!   chain crowdloan.
//! 
//! ## Credits
//! Frederik Schultz <frederik@centrifuge.io>


// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Re-export in crate namespace (for runtime construction)
pub use pallet::*;

// Mock runtime and unit tests
mod mock;
mod tests;

// Runtime benchmarking features
mod benchmarking;

// Extrinsics weight information (computed through runtime benchmarking)
pub mod weights;

//use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::*;
use frame_system::{ensure_root};

use sp_runtime::transaction_validity::{
    InvalidTransaction, 
    ValidTransaction, 
    TransactionValidity
};

use pallet_crowdloan_claim::traits::Reward;


// ----------------------------------------------------------------------------
// Traits declaration
// ----------------------------------------------------------------------------

pub mod traits {

  use frame_support::{
    weights::{Weight}
  };
  
  use frame_support::sp_runtime::traits::{Member, AtLeast32BitUnsigned, MaybeSerializeDeserialize, MaybeSerialize};
  use frame_support::Parameter;
  use frame_support::dispatch::{Codec, DispatchResult};
  use frame_support::dispatch::fmt::Debug;
  use sp_runtime::traits::{MaybeDisplay, Bounded, MaybeMallocSizeOf};
  use sp_runtime::sp_std::hash::Hash;
  use sp_runtime::sp_std::str::FromStr;
  
  
  /// A trait for extrinsincs weight information
  ///
  /// Weights are calculated using runtime benchmarking features.
  /// See [`benchmarking`] module for more information. 
  pub trait WeightInfo {
    fn initialize() -> Weight;
    fn reward() -> Weight;
    fn set_vesting_start() -> Weight;
    fn set_vesting_period() -> Weight;
    fn set_conversion_rate() -> Weight;
    fn set_direct_payout_ratio() -> Weight;
  }
} // end of 'traits' module


/// A type alias for the balance type from this pallet's point of view.
type BalanceOf<T> = <<T as pallet_vesting::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;


// Crowdloan claim pallet module
//
// The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the 
// pallet itself.
#[frame_support::pallet]
pub mod pallet {

  use sp_runtime::traits::{MaybeSerialize};
	use frame_support::{
    dispatch::fmt::Debug,
    pallet_prelude::*,
    traits::{EnsureOrigin}
  };

	use frame_system::pallet_prelude::*;
  pub use crate::traits::WeightInfo;

  // Declare pallet structure placeholder
  #[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);


  // ----------------------------------------------------------------------------
  // Pallet configuration
  // ----------------------------------------------------------------------------

  #[pallet::config]
	/// Generic pallet parameters definition
  pub trait Config: frame_system::Config {

  }
  

  // ----------------------------------------------------------------------------
  // Pallet events
  // ----------------------------------------------------------------------------

  // The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and Codec
  #[pallet::event]
  // The macro generates a function on Pallet to deposit an event
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
  // Additional argument to specify the metadata to use for given type
	#[pallet::metadata(T::AccountId = "AccountId")]
  pub enum Event<T> {
    /// Event emitted when a reward claim was processed successfully.
    /// \[who, direct_reward, vested_reward\]
    RewardClaimed(T::AccountId, BalanceOf<T>, BalanceOf<T>),
  
    /// Event triggered when the reward module is ready to reward contributors
    /// \[vesting_start, vesting_period, conversion_rate, direct_payout_ratio\]
    RewardModuleInitialized(T::BlockNumber, T::BlockNumber, Perbill, Perbill),
  
    /// Direct payout ratio for contributors has been updated
    /// \[payout_ratio\]
    UpdateDirectPayoutRatio(Perbill),
  
    /// Conversion rate from relay to native token has been updated
    UpdateConversionRate(Perbill),
  
    /// Vesting period has been updated
    UpdateVestingPeriod(T::BlockNumber),
  
    /// Start of vesting has been updated
    UpadteVestingStart(T::BlockNumber),
  }


  // ----------------------------------------------------------------------------
  // Pallet storage items
  // ----------------------------------------------------------------------------
  
  /// The conversion rate between relay chain and native chain balances.
  #[pallet::storage]
  #[pallet::getter(fn conversion_rate)]      
  pub(super) type ConversionRate<T: Config>  = StorageValue<_, Perbill, ValueQuery>;
        
  /// Which ratio of the rewards are payed directly. The rest is transferred via a vesting schedule.
  #[pallet::storage]
  #[pallet::getter(fn direct_payout_ratio)]      
  pub(super) type DirectPayoutRatio<T: Config>  = StorageValue<_, Perbill, ValueQuery>;
  
  /// Over which period are the contributions vested.
  #[pallet::storage]
  #[pallet::getter(fn vesting_period)]      
  pub(super) type VestingPeriod<T: Config> = StorageValue<_,T::BlockNumber, ValueQuery>;
        
  /// At which block number does the vesting start.
  #[pallet::storage]
  #[pallet::getter(fn vesting_start)]      
  pub(super) type VestingStart<T: Config> = StorageValue<_,T::BlockNumber, ValueQuery>;

  
  // ----------------------------------------------------------------------------
  // Pallet genesis configuration
  // ----------------------------------------------------------------------------

  // Pallet genesis configuration type definition
  #[pallet::genesis_config]
	#[derive(Default)]
	pub struct GenesisConfig<T: Config> {
    conversion: u32,
    direct_payout: u32
  }

  // Default genesis configuration settings
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
        conversion: Default::default(),
				direct_payout: Default::default()
			}
		}
	}

  // Build of the genesis configuration for the pallet
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
	 	fn build(&self) {
      <ConversionRate<T>>::put(RatePerbill::from_percent(self.conversion));
      <DirectPayoutRatio<T>>::put(RatePerbill::from_percent(self.direct_payout));
		}
	}


  // ----------------------------------------------------------------------------
  // Pallet lifecycle hooks
  // ----------------------------------------------------------------------------
  
  #[pallet::hooks]
	impl<T:Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    // `on_initialize` is executed at the beginning of the block before any extrinsic are
		// dispatched.
		//
		// This function must return the weight consumed by `on_initialize` and `on_finalize`.
		fn on_initialize(_n: T::BlockNumber) -> Weight {
      // TODO:
      // Rreturn a default weight for now. It must be replaced by a weight from
      // WeightInfo
      0
		}

		// `on_finalize` is executed at the end of block after all extrinsic are dispatched.
		fn on_finalize(_n: T::BlockNumber) {
			// clean upd data/state 
		}

		// A runtime code run after every block and have access to extended set of APIs.
		//
		// For instance you can generate extrinsics for the upcoming produced block.
		fn offchain_worker(_n: T::BlockNumber) {
      // nothing done here, folks!
		}
  }


  // ----------------------------------------------------------------------------
  // Pallet errors
  // ----------------------------------------------------------------------------

  #[pallet::error]
	pub enum Error<T> {

    /// Invalid call to an administrative extrinsics
    MustBeAdministrator,

    /// Not enough funds in the pot for paying a reward
    NotEnoughFunds,

    /// Start of vesting period is in the past.
    ElapsedTime
  }


  // ----------------------------------------------------------------------------
  // Pallet dispatchable functions
  // ----------------------------------------------------------------------------

  // Declare Call struct and implement dispatchable (or callable) functions.
  //
  // Dispatchable functions are transactions modifying the state of the chain. They
  // are also called extrinsics are constitute the pallet's public interface.
  // Note that each parameter used in functions must implement `Clone`, `Debug`, 
  // `Eq`, `PartialEq` and `Codec` traits.
  #[pallet::call]
	impl<T:Config> Pallet<T> {

    /// Set start of vesting period.
    ///
    #[pallet::weight(T::WeightInfo::set_vesting_start())]
    pub(crate) fn set_vesting_start(origin: OriginFor<T>, start: T::BlockNumber) -> DispatchResultWithPostInfo {
        T::AdminOrigin::try_origin(origin)
            .map(|_| ())
            .or_else(ensure_root)?;

        ensure!(
            start >= <frame_system::Module<T>>::block_number(),
            Error::<T>::ElapsedTime
        );
        <VestingStart<T>>::put(start);

        Self::deposit_event(Event::UpadteVestingStart(start));

        Ok(().into)
    }

    /// Set vesting period.
    ///
    /// This administrative transaction allows to modify the vesting period
    /// after a previous [`initialize`] transaction was triggered in order
    /// to perform seminal pallet configuration.
    #[pallet::weight(T::WeightInfo::set_vesting_period())]
    pub(crate)fn set_vesting_period(origin: OriginFor<T>, period: T::BlockNumber) -> DispatchResultWithPostInfo {
      // Ensure that only an administrator or root entity triggered the transaction
      ensure!(Self::is_origin_administrator, Error::<T>::MustBeAdministrator);
      
      <VestingPeriod<T>>::put(period);

      Self::deposit_event(Event::UpdateVestingPeriod(period));

      Ok(().into())
    }

    /// Set the rate of conversion between relay and para chains.
    ///
    /// This administrative function allows to set the rate of
    /// conversion between the relay chain and the parachain 
    /// tokens. This dispatchable function is used to modify the
    /// rate of conversion after the pallet has already been
    /// initialized via [`initialize`] transaction.
    #[pallet::weight(T::WeightInfo::set_conversion_rate())]
    pub(crate)fn set_conversion_rate(origin: OriginFor<T>, rate: u32) -> DispatchResultWithPostInfo {
      // Ensure that only an administrator or root entity triggered the transaction
      ensure!(Self::is_origin_administrator, Error::<T>::MustBeAdministrator);

      let rate = Perbill::from_percent(rate);
      <ConversionRate>::put(rate);

      Self::deposit_event(Event::UpdateConversionRate(rate));

      Ok(().into())
    }

    /// Modify the ratio between vested and direct payout amount.
    ///
    /// This administrative function allows to modify the ratio
    /// between vested and direct payout amount after the pallet
    /// was initialized via a call to the [`initialize`] transaction.
    #[pallet::weight(T::WeightInfo::set_direct_payout_ratio())]
    pub(crate) fn set_direct_payout_ratio(origin: OriginFor<T>, ratio: u32) -> DispatchResultWithPostInfo {
      // Ensure that only an administrator or root entity triggered the transaction
      ensure!(Self::is_origin_administrator, Error::<T>::MustBeAdministrator);
      
      let ratio = Perbill::from_percent(ratio);
      
      <DirectPayoutRatio>::put(ratio);

      Self::deposit_event(Event::UpdateDirectPayoutRatio(ratio));

      Ok(().into())
    }
  }
} // end of 'pallet' module


// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Public and privatate functions implementation
impl<T: Config> Pallet<T> {
    
    // Check if a transaction was called by an administrator or root entity.
    fn is_origin_administrator(origin: T::OriginFor<T>) -> DispatchResult {
      T::AdminOrigin::try_origin(origin)
        .map(|_| ())
        .or_else(ensure_root)?;
  
      Ok(())
    }  
}
