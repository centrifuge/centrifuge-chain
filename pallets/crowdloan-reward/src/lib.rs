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
//! Frederik Schulz <frederik@centrifuge.io>

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Runtime, system and frame primitives
use frame_support::{
    dispatch::{fmt::Debug, Codec, DispatchError, DispatchResultWithPostInfo},
    ensure,
    sp_runtime::traits::{AtLeast32BitUnsigned, CheckedMul, CheckedSub, Saturating},
    traits::{
        Currency, EnsureOrigin,
        ExistenceRequirement::{AllowDeath, KeepAlive},
        Get,
    },
    weights::Weight,
};

use frame_system::{ensure_root, RawOrigin};
use sp_runtime::{
    traits::{AccountIdConversion, CheckedDiv, Convert, MaybeSerialize, StaticLookup, Zero},
    ModuleId, Perbill,
};

// Re-export in crate namespace (for runtime construction)
pub use pallet::*;
// Claim reward trait to be implemented
use pallet_crowdloan_claim_reward::Reward;

// Extrinsics weight information
pub use crate::traits::WeightInfo;

// Mock runtime and unit test cases
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// Runtime benchmarking features
#[cfg(test)]
mod benchmarking;

// Extrinsics weight information (computed through runtime benchmarking)
pub mod weights;

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
type BalanceOf<T> = <<T as pallet_vesting::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

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
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use super::*;

    // Declare pallet structure placeholder
    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
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
        /// ```rust,ignore
        ///
        /// // Parameterize crowdloan reward pallet configuration
        /// parameter_types! {
        ///   pub const CrowdloanRewardModuleId: ModuleId = ModuleId(*b"cc/rwrd");
        /// }
        ///
        /// // Implement crowdloan reward pallet's configuration trait for the runtime
        /// impl pallet_crowdloarn_reward::Config for Runtime {
        ///   type Event = Event;
        ///   type WeightInfo = ();
        ///   type ModuleId = CrowdloanRewardModuleId;
        /// }
        ///
        /// ```
        #[pallet::constant]
        type ModuleId: Get<ModuleId>;

        /// Associated type for Event enum
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The balance type of the relay chain
        type RelayChainBalance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + Into<BalanceOf<Self>>;

        // The conversion type, that allows to create a balance-object from a u64
        type Conversion: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug
            + Into<BalanceOf<Self>>
            + From<u64>;

        /// AccountId of the relay chain
        type RelayChainAccountId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeSerialize
            + Ord
            + Default;

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
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    // Additional argument to specify the metadata to use for given type
    #[pallet::metadata(T::AccountId = "AccountId")]
    pub enum Event<T: Config> {
        /// Event emitted when a reward claim was processed successfully.
        /// \[who, direct_reward, vested_reward\]
        RewardClaimed(T::AccountId, BalanceOf<T>, BalanceOf<T>),

        /// Event triggered when the reward module is ready to reward contributors
        /// \[vesting_start, vesting_period, conversion_rate, direct_payout_ratio\]
        RewardModuleInitialized(T::BlockNumber, T::BlockNumber, BalanceOf<T>, Perbill),

        /// Direct payout ratio for contributors has been updated
        /// \[payout_ratio\]
        DirectPayoutRatioUpdated(Perbill),

        /// Conversion rate from relay to native token has been updated
        ConversionRateUpdated(BalanceOf<T>),

        /// Vesting period has been updated
        VestingPeriodUpdated(T::BlockNumber),

        /// Start of vesting has been updated
        VestingStartUpdated(T::BlockNumber),
    }

    // ----------------------------------------------------------------------------
    // Pallet storage items
    // ----------------------------------------------------------------------------

    #[pallet::type_value]
    pub fn OnRateEmpty<T: Config>() -> BalanceOf<T> {
        Into::<BalanceOf<T>>::into(T::Conversion::from(1_000_000u64))
    }

    #[pallet::storage]
    #[pallet::getter(fn conversion_rate)]
    /// The conversion rate between relay chain and native chain balances.
    pub(super) type ConversionRate<T: Config> =
        StorageValue<_, BalanceOf<T>, ValueQuery, OnRateEmpty<T>>;

    #[pallet::type_value]
    pub fn OnRatioEmpty() -> Perbill {
        Perbill::from_percent(20)
    }

    #[pallet::storage]
    #[pallet::getter(fn direct_payout_ratio)]
    /// Which ratio of the rewards are payed directly. The rest is transferred via a vesting schedule.
    pub(super) type DirectPayoutRatio<T: Config> =
        StorageValue<_, Perbill, ValueQuery, OnRatioEmpty>;

    /// Over which period are the contributions vested.
    #[pallet::storage]
    #[pallet::getter(fn vesting_period)]
    pub(super) type VestingPeriod<T: Config> = StorageValue<_, T::BlockNumber>;

    /// At which block number does the vesting start.
    #[pallet::storage]
    #[pallet::getter(fn vesting_start)]
    pub(super) type VestingStart<T: Config> = StorageValue<_, T::BlockNumber>;

    // ----------------------------------------------------------------------------
    // Pallet genesis configuration
    // ----------------------------------------------------------------------------

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

    // ----------------------------------------------------------------------------
    // Pallet lifecycle hooks
    // ----------------------------------------------------------------------------

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // ----------------------------------------------------------------------------
    // Pallet errors
    // ----------------------------------------------------------------------------

    #[pallet::error]
    pub enum Error<T> {
        /// Invalid call to an administrative extrinsics
        MustBeAdministrator,

        /// Not enough funds in the pot for paying a reward
        NotEnoughFunds,

        /// Overflow happened during a mulitplication of balances
        // TODO: Remove with Arithmetic error once we are gone from rococo branch
        Overflow,

        /// Pallet must be initialized first
        PalletNotInitialized,
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
    impl<T: Config> Pallet<T> {
        /// A on call init. Basically a composition of the setters below
        #[pallet::weight(< T as pallet::Config >::WeightInfo::initialize())]
        pub fn initialize(
            origin: OriginFor<T>,
            conversion_rate: BalanceOf<T>,
            direct_payout_ratio: Perbill,
            vesting_period: T::BlockNumber,
            vesting_start: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            ensure!(
                Self::is_origin_administrator(origin).is_ok(),
                Error::<T>::MustBeAdministrator
            );

            <VestingStart<T>>::set(Some(vesting_start));
            <VestingPeriod<T>>::set(Some(vesting_period));
            <DirectPayoutRatio<T>>::put(direct_payout_ratio);
            <ConversionRate<T>>::set(conversion_rate);

            Self::deposit_event(Event::RewardModuleInitialized(
                vesting_start,
                vesting_period,
                conversion_rate,
                direct_payout_ratio,
            ));

            Ok(().into())
        }

        /// Set the start of the vesting period.
        #[pallet::weight(< T as pallet::Config >::WeightInfo::set_vesting_start())]
        pub fn set_vesting_start(
            origin: OriginFor<T>,
            start: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            // Ensure that only an administrator or root entity triggered the transaction
            ensure!(
                Self::is_origin_administrator(origin).is_ok(),
                Error::<T>::MustBeAdministrator
            );

            <VestingStart<T>>::put(start);

            Self::deposit_event(Event::VestingStartUpdated(start));

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
        #[pallet::weight(< T as pallet::Config >::WeightInfo::set_vesting_period())]
        pub fn set_vesting_period(
            origin: OriginFor<T>,
            period: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            // Ensure that only an administrator or root entity triggered the transaction
            ensure!(
                Self::is_origin_administrator(origin).is_ok(),
                Error::<T>::MustBeAdministrator
            );

            <VestingPeriod<T>>::put(period);

            Self::deposit_event(Event::VestingPeriodUpdated(period));

            Ok(().into())
        }

        /// Set the rate of conversion between relay and para chains.
        ///
        /// This administrative function allows to set the rate of
        /// conversion between the relay chain and the parachain
        /// tokens. This dispatchable function is used to modify the
        /// rate of conversion after the pallet has already been
        /// initialized via [`initialize`] transaction.
        #[pallet::weight(< T as pallet::Config >::WeightInfo::set_conversion_rate())]
        pub fn set_conversion_rate(
            origin: OriginFor<T>,
            rate: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            // Ensure that only an administrator or root entity triggered the transaction
            ensure!(
                Self::is_origin_administrator(origin).is_ok(),
                Error::<T>::MustBeAdministrator
            );
            <ConversionRate<T>>::put(rate);

            Self::deposit_event(Event::ConversionRateUpdated(rate));

            Ok(().into())
        }

        /// Modify the ratio between vested and direct payout amount.
        ///
        /// This administrative function allows to modify the ratio
        /// between vested and direct payout amount after the pallet
        /// was initialized via a call to the [`initialize`] transaction.
        #[pallet::weight(< T as pallet::Config >::WeightInfo::set_direct_payout_ratio())]
        pub fn set_direct_payout_ratio(
            origin: OriginFor<T>,
            ratio: Perbill,
        ) -> DispatchResultWithPostInfo {
            // Ensure that only an administrator or root entity triggered the transaction
            ensure!(
                Self::is_origin_administrator(origin).is_ok(),
                Error::<T>::MustBeAdministrator
            );

            <DirectPayoutRatio<T>>::put(ratio);

            Self::deposit_event(Event::DirectPayoutRatioUpdated(ratio));

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
    fn is_origin_administrator(origin: T::Origin) -> DispatchResultWithPostInfo {
        T::AdminOrigin::try_origin(origin)
            .map(|_| ())
            .or_else(ensure_root)?;

        Ok(().into())
    }

    // Convert a contribution in relay chain's token to the parachain's native token
    fn convert_to_native(
        contribution: T::RelayChainBalance,
    ) -> Result<BalanceOf<T>, DispatchError> {
        Self::conversion_rate()
            .checked_mul(&Into::<BalanceOf<T>>::into(contribution))
            .ok_or(Error::<T>::Overflow.into())
    }
}

// ----------------------------------------------------------------------------
// Reward trait implementation
// ----------------------------------------------------------------------------

// Reward trait implementation for the pallet
impl<T: Config> Reward for Pallet<T>
where
    BalanceOf<T>: Send + Sync,
{
    type ParachainAccountId = T::AccountId;
    type ContributionAmount = T::RelayChainBalance;
    type BlockNumber = T::BlockNumber;

    // Reward a payout for a claim on a given parachain account
    fn reward(
        who: Self::ParachainAccountId,
        contribution: Self::ContributionAmount,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            Self::vesting_start().is_some() && Self::vesting_period().is_some(),
            Error::<T>::PalletNotInitialized
        );

        let reward = Self::convert_to_native(contribution)?;
        let from: <T as frame_system::Config>::AccountId = Self::account_id();

        // Ensure transfer will go through and we want to keep the module account alive.
        let free_balance = <T as pallet_vesting::Config>::Currency::free_balance(&from)
            .checked_sub(&<T as pallet_vesting::Config>::Currency::minimum_balance())
            .unwrap_or(Zero::zero());
        ensure!(free_balance > reward, Error::<T>::NotEnoughFunds);

        let direct_reward = Self::direct_payout_ratio() * reward;
        let vested_reward = (Perbill::one().saturating_sub(Self::direct_payout_ratio())) * reward;

        ensure!(
            vested_reward >= <T as pallet_vesting::Config>::MinVestedTransfer::get(),
            pallet_vesting::Error::<T>::AmountLow
        );

        ensure!(
            pallet_vesting::Module::<T>::vesting(&who).is_none(),
            pallet_vesting::Error::<T>::ExistingVestingSchedule
        );

        // Ensure the division is correct or we give everything on the first block
        let per_block = vested_reward
            .checked_div(
                &<<T as pallet_vesting::Config>::BlockNumberToBalance>::convert(
                    Self::vesting_period().unwrap_or(Zero::zero()),
                ),
            )
            .unwrap_or(vested_reward);

        let schedule = pallet_vesting::VestingInfo {
            locked: vested_reward,
            per_block,
            starting_block: Self::vesting_start()
                .unwrap_or(<frame_system::Module<T>>::block_number()),
        };

        let to = <T::Lookup as StaticLookup>::unlookup(who.clone());

        T::Currency::transfer(&from, &who, direct_reward, KeepAlive)?;

        // Currently I know no way to secure that both extrinsic (transfer, vested_transfer)
        // will be successful or be reverted if one of them changes.
        // So, as `vested_transfer` is not revertible, we first transfer the direct amount, and then
        // the vested amount. If first fails, we simply abort. If second fails, we are transferring
        // the direct payout back to the module.
        //
        // NOTE: This procedure does change the state...
        <pallet_vesting::Module<T>>::vested_transfer(
            T::Origin::from(RawOrigin::Signed(from.clone())),
            to,
            schedule,
        )
        .map_err(|err| {
            T::Currency::transfer(&who, &from, direct_reward, AllowDeath)
                .err()
                .unwrap_or_else(|| err)
        })?;

        Self::deposit_event(Event::RewardClaimed(who, direct_reward, vested_reward));

        Ok(().into())
    }
}
