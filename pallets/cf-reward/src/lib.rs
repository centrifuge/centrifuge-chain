//! # Reward module for Centrifuge Parachain Campaigns
//!
//! ## Overview
//!
//! The module does implement the Claim Pallets Reward trait so, that it can be used with alongside.
//! TODO: Describe the rewarding process.
//!
//! ## Callable functions
//!
//! `initialize` - Initializes the module by transfering founds to the modules account and activating an init-lock

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_storage, decl_event, decl_error, dispatch, weights::{Weight}, Parameter, ensure};
use frame_system::{ensure_signed, ensure_root, RawOrigin};
use sp_runtime::traits::{Member, MaybeSerialize, Convert};
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize};
use frame_support::dispatch::{Codec, DispatchError};
use frame_support::dispatch::fmt::Debug;
use frame_support::traits::{Get, Currency, LockableCurrency, LockIdentifier};
use frame_support::dispatch::DispatchResult;
use crowdloan_claim::reward::Reward;
use pallet_vesting::VestingInfo;
use frame_support::storage::*;
use frame_support::traits::ExistenceRequirement::{KeepAlive, AllowDeath};
use sp_runtime::{
    ModuleId,
    traits::{AccountIdConversion, CheckedSub, StaticLookup},
    transaction_validity::{
        TransactionValidity, ValidTransaction, InvalidTransaction, TransactionSource,
        TransactionPriority,
    }
};


#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

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
        NextInit get(fn next_init) : T::BlockNumber;
    }
}


decl_error! {
	pub enum Error for Module<T: Config> {
        /// Not enough funds in the pot for paying a reward payout
        NotEnoughFunds,
        /// Vesting schedule already exists
        AlreadyVested,
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
        /// Basically this call
        ///
        /// Can/Must be weightless as the call must come from root.
        // TODO: Add correct weight function
        #[weight = 10_000]
        fn initialize(origin, source: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;


            let now = <frame_system::Module<T>>::block_number();
            ensure!(now > <NextInit<T>>::get(), Error::<T>::OngoingLease);

            let target = &MODULE_ID.into_account();
            // Transfer payout amount
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
    fn convert_to_rad(contribution: &T::RelayChainBalance) -> <T::Currency as Currency<T::AccountId>>::Balance {
        // TODO: Currently 1:1 conversion
        contribution.clone().into()
    }
}

impl<T: Config> Reward for Module<T> {
    type ParachainAccountId = T::AccountId;
    type ContributionAmount = T::RelayChainBalance;

    fn reward(who: &Self::ParachainAccountId, contribution: &Self::ContributionAmount) -> Result<(), ()> {
        // Create Vesting Schedule
        //
        // - What happens if the Account already has a vesting schedule from another Pallet?
        //      -> This MUST be tested
        // - What happens, when the vesting amount is to little?
        //      -> Direct payout?
        // - What happens when the lease period is over?
        //      -> Install a hook on finalize, that triggers a root signed transaction so that
        //         all balances will be transfered to the contributors. The hook is called each time
        //         checks the current block n and if we are at  n = start_of_lease + time_of_lease -1
        //         then it will trigger

        let schedule = pallet_vesting::VestingInfo {
            locked: Self::convert_to_rad(contribution),
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
                DispatchError::Module { index, error, message } => {
                    // TODO: Create correct error handling that claim module can use:
                    //  If:
                    //    a) Amount to vest to little -> Direct Payout (Risks: Contributors could prevent vesting with many little contributions)
                    //    b) Vesting schedule already exists -> Signal claim, to NOT store account and allow claiming again
                    ()
                },
                _ => (), //Todo:: vest_err,
            }
        })
    }
}