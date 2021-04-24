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

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weight;

use frame_support::{decl_module, decl_storage, decl_event, decl_error,
    Parameter,
    ensure
};
use frame_system::{ensure_root, RawOrigin};
use sp_runtime::traits::{Member, MaybeSerialize, Convert, CheckedDiv};
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Saturating, CheckedSub};
use frame_support::dispatch::Codec;
use frame_support::dispatch::fmt::Debug;
use frame_support::traits::{Get, Currency, ExistenceRequirement::{KeepAlive}, EnsureOrigin};
use frame_support::dispatch::DispatchResult;
use crowdloan_claim::reward::Reward;
use sp_runtime::{ModuleId, traits::{AccountIdConversion, StaticLookup}, Perbill};
pub use weight::WeightInfo;
use sp_runtime::traits::Zero;

pub trait Config: frame_system::Config + pallet_vesting::Config {
    /// This module emits events, and hence, depends on the runtime's definition of event
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

    /// The balance type of the relay chain
    type RelayChainBalance: Parameter + Member + AtLeast32BitUnsigned + Codec + Default + Copy +
        MaybeSerializeDeserialize + Debug +
        Into<BalanceOf<Self>>;

    /// AccountId of the relay chain
    type RelayChainAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeSerialize + Ord
        + Default;

    /// Admin or the module. I.e. this is necessary in cases, where the vesting parameters need
    /// to be changed without an additional initialization.
    type AdminOrigin: EnsureOrigin<Self::Origin>;

    /// Weight information for extrinsics in this pallet
    type WeightInfo: WeightInfo;
}

type BalanceOf<T> = <<T as pallet_vesting::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

const MODULE_ID: ModuleId = ModuleId(*b"rwd/vest");

decl_event!{
    pub enum Event<T> where
        AccountId = <T as frame_system::Config>::AccountId,
        DirectReward = BalanceOf<T>,
        VestedReward = BalanceOf<T>,
        BlockNumber = <T as frame_system::Config>::BlockNumber,
    {
        /// Event emitted when a reward claim was processed successfully.
        RewardClaimed(AccountId, DirectReward, VestedReward),
        /// Event triggered when the reward module is ready to reward contributors
        /// [vesting_start, vesting_period, conversion_rate, direct_payout_ratio]
        RewardModuleInitialized(BlockNumber, BlockNumber, Perbill, Perbill),
        /// Direct payout ratio for contributors has been updated
        UpdateDirectPayoutRatio(Perbill),
        /// Conversion rate from relay to native token has been updated
        UpdateConversionRate(Perbill),
        /// Vesting period has been updated
        UpdateVestingPeriod(BlockNumber),
        /// Start of vesting has been updated
        UpadteVestingStart(BlockNumber),
    }
}

decl_storage! {
    trait Store for Module<T: Config> as Reward {
        /// The conversion rate between relay chain and native chain balances.
        pub ConversionRate get(fn conversion_rate) build(|config: &GenesisConfig| Perbill::from_percent(config.conversion)): Perbill;
        /// Which ratio of the rewards are payed directly. The rest is transferred via a vesting schedule.
        pub DirectPayoutRatio get(fn direct_payout_ratio) build(|config: &GenesisConfig| Perbill::from_percent(config.direct_payout)): Perbill;
        /// Over which period are the contributions vested.
        pub VestingPeriod get(fn vesting_period) : T::BlockNumber;
        /// At which block number does the vesting start .
        pub VestingStart get(fn vesting_start) : T::BlockNumber;
    } add_extra_genesis {
        config(direct_payout) : u32;
        config(conversion) : u32;
    }
}

decl_error! {
	pub enum Error for Module<T: Config> {
        /// Not enough funds in the pot for paying a reward
        NotEnoughFunds,
        /// Start of vesting period is in the past.
        ElapsedTime,
    }
}


decl_module! {
    pub struct Module<T: Config> for enum Call where origin: <T as frame_system::Config>::Origin {
        // Activate errors
        type Error = Error<T>;

        const MODULE_ACCOUNT_ID: <T as frame_system::Config>::AccountId = MODULE_ID.into_account();

        // Activate events
        fn deposit_event() = default;

        #[weight = <T as Config>::WeightInfo::set_vesting_start()]
        fn set_vesting_start(origin, start: T::BlockNumber) -> DispatchResult {
            T::AdminOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)?;

            ensure!(
                start >= <frame_system::Module<T>>::block_number(),
                Error::<T>::ElapsedTime
            );
            <VestingStart<T>>::put(start);

            Self::deposit_event(RawEvent::UpadteVestingStart(start));

            Ok(())
        }

        #[weight = <T as Config>::WeightInfo::set_vesting_period()]
        fn set_vesting_period(origin, period: T::BlockNumber) -> DispatchResult {
            T::AdminOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)?;
            <VestingPeriod<T>>::put(period);

            Self::deposit_event(RawEvent::UpdateVestingPeriod(period));

            Ok(())
        }

        #[weight = <T as Config>::WeightInfo::set_conversion_rate()]
        fn set_conversion_rate(origin, rate: u32) -> DispatchResult {
            T::AdminOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)?;
            let rate = Perbill::from_percent(rate);
            <ConversionRate>::put(rate);

            Self::deposit_event(RawEvent::UpdateConversionRate(rate));

            Ok(())
        }

        #[weight = <T as Config>::WeightInfo::set_direct_payout_ratio()]
        fn set_direct_payout_ratio(origin, ratio: u32) -> DispatchResult {
            T::AdminOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)?;
            let ratio = Perbill::from_percent(ratio);
            <DirectPayoutRatio>::put(ratio);

            Self::deposit_event(RawEvent::UpdateDirectPayoutRatio(ratio));

            Ok(())
        }
    }
}

impl<T: Config> Module<T> {
    fn convert_to_native(contribution: T::RelayChainBalance) -> BalanceOf<T> {
        //(contribution.into()/Self::full_percent()) * Self::conversion_rate()
        let contribution = Into::<BalanceOf<T>>::into(contribution);
        Self::conversion_rate() *  contribution
    }
}

impl<T: Config> Reward for Module<T>
    where BalanceOf<T>: Send + Sync
{
    type ParachainAccountId = T::AccountId;
    type ContributionAmount = T::RelayChainBalance;
    type BlockNumber = T::BlockNumber;

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
        <DirectPayoutRatio>::put(ratio);

        let rate = Perbill::from_percent(conversion_rate);
        <ConversionRate>::set(rate);

        Self::deposit_event(RawEvent::RewardModuleInitialized(
            vesting_start,
            vesting_period,
            rate,
            ratio)
        );

        Ok(())
    }

    fn reward(who: Self::ParachainAccountId, contribution: Self::ContributionAmount) -> DispatchResult {
        let reward = Self::convert_to_native(contribution);
        let from: <T as frame_system::Config>::AccountId = MODULE_ID.into_account();

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

        Self::deposit_event(RawEvent::RewardClaimed(who, direct_reward, vested_reward));

        Ok(())
    }
}