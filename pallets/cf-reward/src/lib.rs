// Copyright 2019-2021 Centrifuge Inc.
// This file is part of Cent-Chain.

// Cent-Chain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cent-Chain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cent-Chain.  If not, see <http://www.gnu.org/licenses/>.

//! # Reward module for Centrifuge Parachain Campaigns
//!
//! ## Overview
//! The function of this pallet is to provide the centrifuge specific reward functionality for
//! contributors of the relay chain crowdloan. In order to provide this functionality the pallet
//! implements the `Reward` Trait from the `Claim` Pallet.
//! Before any rewards can be provided to contributors the pallet MUST be initialized, so that the
//! modules account holds the necessary funds to reward.
//! All rewards are payed in form of a `vested_transfer` with a fixed vesting time. The vesting time
//! is defined via the modules `Config` trait.
//!
//! ## Callable Functions
//!
//! - `initialize` - Initializes the module by transfering founds to the modules account and activating an init-lock
//!
//! ## Implementations
//!
//! - [`Reward`](crowdloan_claim::reward::Reward) : Rewarding functionality for contributors in a relay
//!   chain crowdloan.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_storage, decl_event, decl_error, weights::{Weight}, Parameter, ensure};
use frame_system::{ensure_root, RawOrigin};
use sp_runtime::traits::{Member, MaybeSerialize, Convert};
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize};
use frame_support::dispatch::{Codec, DispatchError};
use frame_support::dispatch::fmt::Debug;
use frame_support::traits::{Get, Currency, ExistenceRequirement::{AllowDeath }};
use frame_support::dispatch::DispatchResult;
use crowdloan_claim::reward::Reward;
use sp_runtime::{ModuleId, traits::{AccountIdConversion, StaticLookup}};


#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Config: frame_system::Config + pallet_vesting::Config {
    /// Timer after which the Module can be initalized again. This should probably be less than
    /// the maximum lock-time.
    type LeasePeriod: Get<Self::BlockNumber>;
    /// Period of vesting for every contributor
    type VestingPeriod: Get<Self::BlockNumber>;
    /// This module emits events, and hence, depends on the runtime's definition of event
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    /// The balance type of the relay chain
    type RelayChainBalance: Parameter + Member + AtLeast32BitUnsigned + Codec + Default + Copy +
        MaybeSerializeDeserialize + Debug +
        Into<<<Self as pallet_vesting::Config>::Currency as Currency<<Self as frame_system::Config>::AccountId>>::Balance>;
    /// AccountId of the relay chain
    type RelayChainAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeSerialize + Ord
        + Default;
}

type BalanceOf<T> = <<T as pallet_vesting::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

const MODULE_ID: ModuleId = ModuleId(*b"rewards ");

/// Callable functions (i.e. transaction) weight trait
pub trait WeightInfo {
    fn initialize() -> Weight;
    fn reward() -> Weight;
}

// Define transaction weights to be used when testing
pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
    fn initialize() -> Weight { 0 }
    fn reward() -> Weight { 0 }
}


decl_event!{
    pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
        /// Event emitted when a reward claim was processed successfully.
        RewardClaimed(AccountId),
        /// Event triggered when the reward module is ready to reward contributors
        RewardModuleInitalized,
    }
}

decl_storage! {
    trait Store for Module<T: Config> as Reward {
        NextInit get(fn next_init) config(): T::BlockNumber;
        ModuleAccount get(fn mod_account) : T::AccountId = MODULE_ID.into_account();
        ConversionRate get(fn conversion_rate) config(): BalanceOf<T>;
    }
}

decl_error! {
	pub enum Error for Module<T: Config> {
        /// Not enough funds in the pot for paying a reward
        NotEnoughFunds,
        /// Initalization before the last lease period is over
        OngoingLease,
    }
}


decl_module! {
    pub struct Module<T: Config> for enum Call where origin: <T as frame_system::Config>::Origin {
        // Activate errors
        type Error = Error<T>;

        // Activate events
        fn deposit_event() = default;

        /// Initialize the module.
        /// This dispatchable function must be called from a root account, so that other accounts
        /// can not block the module via a random initialization. Furthermore it is important, that
        /// the module has enough funds, as a later funding is not possible.
        // TODO: Add correct weight function
        #[weight = 10_000]
        fn initialize(origin, source: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;

            let now = <frame_system::Module<T>>::block_number();
            ensure!(now > <NextInit<T>>::get(), Error::<T>::OngoingLease);

            let target = &MODULE_ID.into_account();
            // TODO: Should we choose: KeepAlive or AllowDeath here?
            T::Currency::transfer(
                &source,
                &target,
                amount,
                AllowDeath,
            )?;

            <NextInit<T>>::put(now + T::LeasePeriod::get());

            Self::deposit_event(RawEvent::RewardModuleInitalized);

            Ok(())
        }
    }
}

impl<T: Config> Module<T> {
    fn convert_to_native(contribution: T::RelayChainBalance) -> <T::Currency as Currency<T::AccountId>>::Balance {
        contribution.into() * <ConversionRate<T>>::get()
    }
}

impl<T: Config> Reward for Module<T> {
    type ParachainAccountId = T::AccountId;
    type ContributionAmount = T::RelayChainBalance;

    fn reward(who: Self::ParachainAccountId, contribution: Self::ContributionAmount) -> DispatchResult {
        let schedule = pallet_vesting::VestingInfo {
            locked: Self::convert_to_native(contribution),
            per_block: <<T as pallet_vesting::Config>::BlockNumberToBalance>::convert(T::VestingPeriod::get()),
            starting_block: <frame_system::Module<T>>::block_number(),
        };

        let from: <T as frame_system::Config>::AccountId = MODULE_ID.into_account();
        let target = <T::Lookup as StaticLookup>::unlookup(who.clone());

        <pallet_vesting::Module<T>>::vested_transfer(
            T::Origin::from(RawOrigin::Signed(from)),
            target,
            schedule
        ).map_err(|vest_err| {
            match vest_err {
                DispatchError::Module { index: _, error: _, message } => {
                    message.map_or(vest_err, |msg| {
                        match msg {
                            "InsufficientBalance" => Into::<DispatchError>::into(Error::<T>::NotEnoughFunds),
                            "AmountLow" => { vest_err
                                // TODO: Should we do this? Or should we set MinVestAmount = 1?
                                /*T::Currency::transfer(
                                    &from,
                                    &target,
                                    Self::convert_to_native(contribution),
                                    ExistenceRequirement::AllowDeath,
                                )*/
                            },
                            _ => vest_err,
                        }
                    })

                },
                _ => vest_err,
            }
        }).map(|_| {
            Self::deposit_event(RawEvent::RewardClaimed(who));
        })
    }
}