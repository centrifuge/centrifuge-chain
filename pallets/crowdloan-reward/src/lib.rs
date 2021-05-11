
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
//! This pallet implements a specific rewarding strategy for crowdloan
//! campaign. It worth pointing out that this pallet works hand in hand
//! with the [`pallet-crowdloan-claim`] pallet, the latter being responsible
//! for managing reward claims (including security aspects, such as malicious
//! Denial of Service or replay attacks).
//!
//! - \[`Config`]
//! - \[`Call`]
//! - \[`Pallet`]
//! - \[`Reward`](pallet_crowdloan_claim::traits::Reward)
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
//! ## Terminology
//! For information on terms and concepts used in this pallet,
//! please refer to the [pallet' specification document](https://centrifuge.hackmd.io/JIGbo97DSiCPFnBFN62aTQ?both).
//!
//! ## Goals
//! 
//! ## Usage
//!
//! ## Interface
//!
//! ### Supported Origins
//! Valid origin is an administrator or root.
//!
//! ### Types
//!
//! ### Events
//!
//! ### Errors
//!
//! ### Dispatchable Functions
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
//! ### Public Functions
//!
//! ## Genesis Configuration
//!
//! ## Dependencies
//! This pallet works hand in hand with [`pallet-crowdloan-claim`] pallet. In fact, it must
//! implement this pallet's [`pallet-crowdloan-claim::traits::Reward`] trait so that to interact.
//!
//! ## References
//! 
//! ## Credits
//! Frederik Schultz <frederik@centrifuge.io>


// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Re-export in crate namespace (for runtime construction)
pub use pallet::*;

// Mock runtime and unit test cases
mod mock;
mod tests;

// Runtime benchmarking features
mod benchmarking;

// Extrinsics weight information (computed through runtime benchmarking)
pub mod weights;

// Runtime, system and frame primitives
use frame_support::{
  dispatch::{
    Codec,
    DispatchResult,
    fmt::Debug,
  },
  ensure,
  Parameter, 
  sp_runtime::traits::{
    AtLeast32BitUnsigned, 
    CheckedSub,
    MaybeSerializeDeserialize, 
    Saturating, 
  }, 
  traits::{
    Get, 
    Currency, 
    ExistenceRequirement::KeepAlive, 
    EnsureOrigin
  }, 
  weights::Weight
};

use frame_system::{
  ensure_root,
  RawOrigin
};

use sp_runtime::{
  ModuleId,
  Perbill,
  traits::{
    AccountIdConversion,
    Convert, 
    CheckedDiv,
    MaybeSerialize, 
    Member,
    StaticLookup,    
    Zero,
  }
};

// Claim reward trait to be implemented
use pallet_crowdloan_claim::traits::RewardMechanism;

// Extrinsics weight information
pub use crate::traits::WeightInfo;


// ----------------------------------------------------------------------------
// Traits and types declaration
// ----------------------------------------------------------------------------

pub mod traits {

  use super::*;
  
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


// ----------------------------------------------------------------------------
// Pallet module
// ----------------------------------------------------------------------------

// Crowdloan claim pallet module
//
// The name of the pallet is provided by `construct_runtime` macro and is used
// as the unique identifier for the pallet's storage. It is not defined in the 
// pallet itself.
#[frame_support::pallet]
pub mod pallet {

	use super::*;
  use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
  
  // Declare pallet structure placeholder
  #[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);


  // ----------------------------------------------------------------------------
  // Pallet configuration
  // ----------------------------------------------------------------------------

  #[pallet::config]
	pub trait Config: frame_system::Config + pallet_vesting::Config {
    /// Constant configuration parameter to store the module identifier for the pallet.
    ///
    /// The module identifier may be of the form ```ModuleId(*b"cc/rwrd")```. This
    /// constant is set when building this config trait for the runtime.
    ///
    /// # Example
    /// ```rust
    /// …
    /// // Parameterize crowdloan reward pallet configuration
    /// parameter_types! {
    ///   pub const CrowdloanRewardModuleId: ModuleId = ModuleId(*b"cc/rewrd");
    /// }
    ///
    /// // Implement crowdloan reward pallet's configuration trait for the runtime
    /// impl pallet_crowdloarn_reward::Config for Runtime {
    ///   type Event = Event;
    ///   type WeightInfo = ();
    ///   type ModuleId = CrowdloanRewardModuleId;
    /// }
    /// …
    /// ```
    #[pallet::constant]
    type ModuleId: Get<ModuleId>;
    
    /// Associated type for Event enum
    type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

    /// The balance type of the relay chain
    type RelayChainBalance: Parameter + Member + 
      AtLeast32BitUnsigned + Codec + Default + Copy +
      MaybeSerializeDeserialize + Debug +
      Into<BalanceOf<Self>>;

    /// AccountId of the relay chain
    type RelayChainAccountId: Parameter + Member + 
      MaybeSerializeDeserialize + Debug + 
      MaybeSerialize + Ord +
      Default;

    /// Admin or the module. I.e. this is necessary in cases, where the vesting parameters need
    /// to be changed without an additional initialization.
    type AdminOrigin: EnsureOrigin<Self::Origin>;

    /// Weight information for extrinsics in this pallet
    type WeightInfo: WeightInfo;
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
  pub enum Event<T: Config> {
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
    UpdateVestingStart(T::BlockNumber),
  }


  // ----------------------------------------------------------------------------
  // Pallet storage items
  // ----------------------------------------------------------------------------
  
  /// The conversion rate between relay chain and native chain balances.
  #[pallet::storage]
  #[pallet::getter(fn conversion_rate)]      
  pub(super) type ConversionRate<T: Config> = StorageValue<_, Perbill, ValueQuery> ;
        
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
	
  /// Pallet genesis configuration type declaration.
  ///
  /// It allows to build genesis storage.
  // #[pallet::genesis_config]
	// pub struct GenesisConfig {
  //   #[doc = "Conversion rate between relay chain and parachain tokens."]
  //   pub conversion: u32,
  //   #[doc = "Direct reward payout ratio."]
  //   pub direct_payout: u32
  // }

  // The default value for the genesis config type.
	// #[cfg(feature = "std")]
	// impl Default for GenesisConfig {
	// 	fn default() -> Self {
	// 		Self {
	// 			conversion: 80,
  //       direct_payout: 20,
	// 		}
	// 	}
	// }

  // The build of genesis configuration for the pallet.
	// #[pallet::genesis_build]
	// impl<T: Config> GenesisBuild<T> for GenesisConfig {
	//  	fn build(&self) {
  //     <ConversionRate<T>>::put(Perbill::from_percent(self.conversion));
  //     <DirectPayoutRatio<T>>::put(Perbill::from_percent(self.direct_payout));
	// 	}
	// }


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
			// clean up data/state 
		}

		// A runtime code run after every block and have access to extended set of APIs.
		//
		// For instance you can generate extrinsics for the upcoming produced block.
		fn offchain_worker(_n: T::BlockNumber) {
      // nothing done here, folks!
		}

		fn on_runtime_upgrade() -> Weight { 0 }

		fn integrity_test() {}
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

    /// Set the start of the vesting period.
    #[pallet::weight(<T as pallet::Config>::WeightInfo::set_vesting_start())]
    pub(crate) fn set_vesting_start(origin: OriginFor<T>, start: T::BlockNumber) -> DispatchResultWithPostInfo {
      // Ensure that only an administrator or root entity triggered the transaction
      ensure!(Self::is_origin_administrator(origin) == Ok(()), Error::<T>::MustBeAdministrator);

      ensure!(
        start >= <frame_system::Module<T>>::block_number(),
        Error::<T>::ElapsedTime
      );

      <VestingStart<T>>::put(start);

      Self::deposit_event(Event::UpdateVestingStart(start));

      Ok(().into())
    }

    /// Set vesting period.
    ///
    /// This administrative transaction allows to modify the vesting period
    /// after a previous [`initialize`] transaction was triggered in order
    /// to perform seminal pallet configuration.
    ///
    /// ## Emits
    /// UpdateVestingPeriod
    #[pallet::weight(<T as pallet::Config>::WeightInfo::set_vesting_period())]
    pub(crate)fn set_vesting_period(origin: OriginFor<T>, period: T::BlockNumber) -> DispatchResultWithPostInfo {
      // Ensure that only an administrator or root entity triggered the transaction
      ensure!(Self::is_origin_administrator(origin) == Ok(()), Error::<T>::MustBeAdministrator);
      
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
    #[pallet::weight(<T as pallet::Config>::WeightInfo::set_conversion_rate())]
    pub(crate)fn set_conversion_rate(origin: OriginFor<T>, rate: u32) -> DispatchResultWithPostInfo {
      // Ensure that only an administrator or root entity triggered the transaction
      ensure!(Self::is_origin_administrator(origin) == Ok(()), Error::<T>::MustBeAdministrator);

      let rate = Perbill::from_percent(rate);
      <ConversionRate<T>>::put(rate);

      Self::deposit_event(Event::UpdateConversionRate(rate));

      Ok(().into())
    }

    /// Modify the ratio between vested and direct payout amount.
    ///
    /// This administrative function allows to modify the ratio
    /// between vested and direct payout amount after the pallet
    /// was initialized via a call to the [`initialize`] transaction.
    #[pallet::weight(<T as pallet::Config>::WeightInfo::set_direct_payout_ratio())]
    pub(crate) fn set_direct_payout_ratio(origin: OriginFor<T>, ratio: u32) -> DispatchResultWithPostInfo {
      // Ensure that only an administrator or root entity triggered the transaction
      ensure!(Self::is_origin_administrator(origin) == Ok(()), Error::<T>::MustBeAdministrator);
      
      let ratio = Perbill::from_percent(ratio);
      
      <DirectPayoutRatio<T>>::put(ratio);

      Self::deposit_event(Event::UpdateDirectPayoutRatio(ratio));

      Ok(().into())
    }
  }
} // end of 'pallet' module


// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Pallet implementation block.
//
// This main implementation block contains two categories of functions, namely:
//
// - Public functions: These are functions that are `pub` and generally fall
//   into inspector functions (i.e. immutables) that do not write to storage 
//   and operation functions that do (i.e. mutables).
//
// - Private functions: These are private helpers or utilities that cannot be
//   called from other pallets.
impl<T: Config> Pallet<T> {

  /// Return the account identifier of the crowdloan reward pallet.
	///
	/// This actually does computation. If you need to keep using it, then make
	/// sure you cache the value and only call this once.
	pub fn account_id() -> T::AccountId {
	  T::ModuleId::get().into_account()
	}

  // Check if a transaction was called by an administrator or root entity.
  fn is_origin_administrator(origin: T::Origin) -> DispatchResult {
    T::AdminOrigin::try_origin(origin)
      .map(|_| ())
      .or_else(ensure_root)?;

    Ok(())
  }  

  // Convert a contribution in relay chain's token to the parachain's native token
  fn convert_to_native(contribution: T::RelayChainBalance) -> BalanceOf<T> {
    //(contribution.into()/Self::full_percent()) * Self::conversion_rate()
    let contribution = Into::<BalanceOf<T>>::into(contribution);
    Self::conversion_rate() *  contribution
  }
}


// ----------------------------------------------------------------------------
// Reward trait implementation
// ----------------------------------------------------------------------------

// Reward trait implementation for the pallet
impl<T: Config> RewardMechanism for Pallet<T>
  where BalanceOf<T>: Send + Sync
{
  type ParachainAccountId = T::AccountId;
  type ContributionAmount = T::RelayChainBalance;
  type BlockNumber = T::BlockNumber;

  // Configure reward pallet
  fn initialize(
      conversion_rate: u32,
      direct_payout_ratio: u32,
      vesting_period: Self::BlockNumber,
      vesting_start: Self::BlockNumber
  ) -> DispatchResult
  {
    ensure!(
      vesting_start >= <frame_system::Module<T>>::block_number(),
      Error::<T>::ElapsedTime
    );

    <VestingStart<T>>::set(vesting_start);
    <VestingPeriod<T>>::set(vesting_period);

    let ratio = Perbill::from_percent(direct_payout_ratio);
    <DirectPayoutRatio<T>>::put(ratio);

    let rate = Perbill::from_percent(conversion_rate);
    <ConversionRate<T>>::set(rate);

    Self::deposit_event(Event::RewardModuleInitialized(
        vesting_start,
        vesting_period,
        rate,
        ratio)
    );

    Ok(())
  }

  // Reward a payout for a claim on a given parachain account
  fn reward(who: Self::ParachainAccountId, contribution: Self::ContributionAmount) -> DispatchResult {
    let reward = Self::convert_to_native(contribution);
    let from: <T as frame_system::Config>::AccountId = Self::account_id();
    
    // Ensure transfer will go through and we want to keep the module account alive.
    let free_balance = <T as pallet_vesting::Config>::Currency::free_balance(&from)
      .checked_sub(&<T as pallet_vesting::Config>::Currency::minimum_balance()).unwrap_or(Zero::zero());
    ensure!( free_balance > reward, Error::<T>::NotEnoughFunds );

    let direct_reward = Self::direct_payout_ratio() * reward;
    let vested_reward = (Perbill::one().saturating_sub(Self::direct_payout_ratio())) * reward;

    ensure!(
      vested_reward >= <T as pallet_vesting::Config>::MinVestedTransfer::get(),
      pallet_vesting::Error::<T>::AmountLow
    );

    // Ensure the division is correct or we give everything on the first block
    let per_block = vested_reward.checked_div(&<<T as pallet_vesting::Config>::BlockNumberToBalance>::convert(Self::vesting_period()))
      .unwrap_or(vested_reward);
    
      let schedule = pallet_vesting::VestingInfo {
      locked: vested_reward,
      per_block,
      starting_block: Self::vesting_start()
    };

    let to = <T::Lookup as StaticLookup>::unlookup(who.clone());

    <pallet_vesting::Module<T>>::vested_transfer(
      T::Origin::from(RawOrigin::Signed(from.clone())),
      to,
      schedule
    )?;

    T::Currency::transfer(&from, &who, direct_reward, KeepAlive)?;

    Self::deposit_event(Event::RewardClaimed(who, direct_reward, vested_reward));

    Ok(())
  }
}