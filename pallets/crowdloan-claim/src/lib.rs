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

//! # Crowdloan claim pallet
//!
//! A pallet used for claiming reward payouts for crowdloan campaigns. This
//! does not implement the rewarding strategy, that is the role of the
//! [`pallet-crowdloan-reward`] pallet.
//!
//! - \[`Config`]
//! - \[`Call`]
//! - \[`Pallet`]
//!
//! ## Overview
//! This pallet is used to proces reward claims from contributors who locked
//! tokens on the Polkadot/Kusama relay chain for participating in a crowdloan
//! campaign for a parachain slot acquisition.
//!
//! This module is intimately bound to the [`pallet-crowdloan-reward`] pallet, where
//! the rewarding strategy is implemented.
//!
//! ## Terminology
//! For information on terms and concepts used in this pallet,
//! please refer to the pallet' specification document.
//!
//! ## Goals
//! The aim of this pallet is to ensure that a contributor who's claiming a reward
//! is eligible for it and avoid potential attacks, such as, for instance, Denial of
//! Service (DoS) or spams (e.g. massive calls of unsigned claim transactions, that
//! are free of charge and can be used as a vector to lock down the network.
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
//! Callable functions (or extrinsics), also considered as transactions, materialize the
//! pallet contract. Here's the callable functions implemented in this module:
//!
//! [`claim_reward`] Note that this extrinsics is invoked via an unsigned (and hence feeless)
//! transactions. A throttling mechanism for slowing down transactions beat exists, so that
//! to prevent massive malicious claims that could potentially impact the network.
//!
//! ### Public Functions
//!
//! ## Genesis Configuration
//!
//! ## Dependencies
//! As stated in the overview of this pallet, the latter relies on the [`pallet-crowdloan-reward`]
//! pallet to implement the specific rewarding strategy of a parachain.
//! A rewarding pallet must implement the [`pallet-crowdloan-claim::traits::Reward`] trait that is
//! declared in this pallet.
//! For this pallet to be able to interact with a reward pallet, it must be loosely coupled to the
//! former using an associated type which includes the [`pallet-crowdloan-claim::traits::Reward`]
//! trait.
//!
//! ## References
//! - [Building a Custom Pallet](https://substrate.dev/docs/en/tutorials/build-a-dapp/pallet). Retrieved April 5th, 2021.
//!
//! ## Credits
//! The Centrifugians Tribe <tribe@centrifuge.io>

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------
#[macro_use]
extern crate lazy_static;

use codec::{Decode, Encode};
// Runtime, system and frame primitives
use frame_support::{
    dispatch::{fmt::Debug, Codec, DispatchResult},
    ensure,
    sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerialize, MaybeSerializeDeserialize, Member},
    traits::{EnsureOrigin, Get},
    weights::Weight,
    Parameter,
};
use frame_system::ensure_root;
use sp_core::crypto::AccountId32;
use sp_core::storage::ChildInfo;
use sp_core::Hasher;
use sp_runtime::{
    sp_std::{hash::Hash, str::FromStr, vec, vec::Vec},
    traits::{AccountIdConversion, Bounded, MaybeDisplay, MaybeMallocSizeOf, Verify, Zero},
    ModuleId, MultiSignature,
};
use sp_state_machine::read_child_proof_check;
use sp_std::convert::TryInto;
use sp_trie::StorageProof;

// Re-export in crate namespace (for runtime construction)
pub use pallet::*;

// Extrinsics weight information
pub use crate::traits::WeightInfo as PalletWeightInfo;

// Mock runtime and unit test cases
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// Extrinsics weight information (computed through runtime benchmarking)
pub mod weights;

// ----------------------------------------------------------------------------
// Traits and types declaration
// ----------------------------------------------------------------------------

pub mod traits {
    use sp_runtime::traits::Zero;

    use super::*;
    use frame_support::sp_runtime::Perbill;

    /// Weight information for pallet extrinsics
    ///
    /// Weights are calculated using runtime benchmarking features.
    /// See [`benchmarking`] module for more information.
    pub trait WeightInfo {
        fn initialize() -> Weight;
        fn claim_reward() -> Weight;
    }

    /// A trait used for loosely coupling the claim pallet with a reward mechanism.
    ///
    /// ## Overview
    /// The crowdloan reward mechanism is separated from the crowdloan claiming process, the latter
    /// being generic, acting as a kind of proxy to the rewarding mechanism, that is specific to
    /// to each crowdloan campaign. The aim of this pallet is to ensure that a claim for a reward
    /// payout is well-formed, checking for replay attacks, spams or invalid claim (e.g. unknown
    /// contributor, exceeding reward amount, ...).
    /// See the [`crowdloan-reward`] pallet, that implements a reward mechanism with vesting, for
    /// instance.
    ///
    /// ## Example
    /// ```rust
    ///
    /// ```
    pub trait Reward {
        /// The account from the parachain, that the claimer provided in her/his transaction.
        type ParachainAccountId: Debug
            + Default
            + MaybeSerialize
            + MaybeSerializeDeserialize
            + Member
            + Ord
            + Parameter;

        /// The contribution amount in relay chain tokens.
        type ContributionAmount: AtLeast32BitUnsigned
            + Codec
            + Copy
            + Debug
            + Default
            + MaybeSerializeDeserialize
            + Member
            + Parameter
            + Zero;

        /// Block number type used by the runtime
        type BlockNumber: AtLeast32BitUnsigned
            + Bounded
            + Copy
            + Debug
            + Default
            + FromStr
            + Hash
            + MaybeDisplay
            + MaybeMallocSizeOf
            + MaybeSerializeDeserialize
            + Member
            + Parameter;

        type NativeBalance: Parameter
            + Member
            + AtLeast32BitUnsigned
            + Codec
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + Debug;

        /// Rewarding function that is invoked from the claim pallet.
        ///
        /// If this function returns successfully, any subsequent claim of the same claimer will be
        /// rejected by the claim module.
        fn reward(
            who: Self::ParachainAccountId,
            contribution: Self::ContributionAmount,
        ) -> DispatchResult;

        /// Initialize function that will be called during the initialization of the crowdloan claim pallet.
        ///
        /// The main purpose of this function is to allow a dynamic configuration of the crowdloan reward
        /// pallet.
        fn initialize(
            conversion_rate: Self::NativeBalance,
            direct_payout_ratio: Perbill,
            vesting_period: Self::BlockNumber,
            vesting_start: Self::BlockNumber,
        ) -> DispatchResult;
    }
} // end of 'traits' module

/// A type alias for the balance type from this pallet's point of view.
type BalanceOf<T> = <T as pallet_balances::Config>::Balance;

/// A type alias for crowdloan's child trie root hash, from this claim pallet's point of view.
///
/// When setting up the pallet via the [`initialize`] transaction, the
/// child trie root hash containing all contributions, is transfered from
/// the [`crowdloan`] pallet' storage to [`Contributions`] storage item
// of this pallet.
/// The [`Contributions`] root hash is used to check if a contributor is
/// eligible for a reward payout and to get the amount of her/his contribution
/// (in relay chain's native token) to a crowdloan campaign.
type ChildTrieRootHashOf<T> = <T as frame_system::Config>::Hash;

/// A type alias for the parachain account identifier from this claim pallet's point of view
type ParachainAccountIdOf<T> =
    <<T as Config>::RewardMechanism as traits::Reward>::ParachainAccountId;

/// A type alias for the contribution amount (in relay chain tokens) from this claim pallet's point of view
type ContributionAmountOf<T> =
    <<T as Config>::RewardMechanism as traits::Reward>::ContributionAmount;

/// Index of the crowdloan campaign inside the
/// [crowdloan.rs](https://github.com/paritytech/polkadot/blob/77b3aa5cb3e8fa7ed063d5fbce1ae85f0af55c92/runtime/common/src/crowdloan.rs#L80)
/// on polkadot.
type TrieIndex = u32;

// ----------------------------------------------------------------------------
// Pallet module
// ----------------------------------------------------------------------------

// Crowdloan claim pallet module
//
// The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the
// pallet itself.
#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use crate::traits::Reward;

    use super::*;

    // Crowdloan claim pallet type declaration.
    //
    // This structure is a placeholder for traits and functions implementation
    // for the pallet.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // ------------------------------------------------------------------------
    // Pallet configuration
    // ------------------------------------------------------------------------

    /// Crowdloan claim pallet's configuration trait.
    ///
    /// Associated types and constants are declared in this trait. If the pallet
    /// depends on other super-traits, the latter must be added to this trait,
    /// such as, in this case, [`frame_system::Config`] and [`pallet_balances::Config`]
    /// super-traits. Note that [`frame_system::Config`] must always be included.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// Associated type for Event enum
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Constant configuration parameter to store the module identifier for the pallet.
        ///
        /// The module identifier may be of the form ```ModuleId(*b"cc/claim")```.
        #[pallet::constant]
        type ModuleId: Get<ModuleId>;

        /// Contributor's account identifier on the relay chain.
        type RelayChainAccountId: Debug
            + Default
            + MaybeSerialize
            + MaybeSerializeDeserialize
            + Member
            + Ord
            + Parameter
            + Into<AccountId32>;

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

        /// Priority of the unsigned claim transaction.
        ///
        /// Since the claim transaction is unsigned, a mechanism must ensure that
        /// it cannot be used for forging a malicious denial of service attack.
        /// This priority property can be tweaked, according to the runtime
        /// specificities (using `parameters_type` macro). The higher the value,
        /// the most prioritized is the transaction.
        type ClaimTransactionPriority: Get<TransactionPriority>;

        /// Longevity of the unsigned claim transaction.
        ///
        /// This parameter indicates the minimum number of blocks that the
        /// claim transaction will remain valid for.
        /// The [`TransactionLongevity::max_value()`] means "forever".
        /// This property is used to prevent the unsigned claim transaction
        /// from being used as a vector for a denial of service attack.
        type ClaimTransactionLongevity: Get<Self::BlockNumber>;

        /// The reward payout mechanism this claim pallet uses.
        ///
        /// This associated type allows to implement a loosely-coupled regime between
        /// claiming and rewarding pallets.
        type RewardMechanism: Reward;

        /// Entity which is allowed to perform administrative transactions
        type AdminOrigin: EnsureOrigin<Self::Origin>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: PalletWeightInfo;
    }

    // ------------------------------------------------------------------------
    // Pallet events
    // ------------------------------------------------------------------------

    // The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and Codec
    #[pallet::event]
    // The macro generates a function on Pallet to deposit an event
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    // Additional argument to specify the metadata to use for given type
    #[pallet::metadata(T::AccountId = "AccountId")]
    pub enum Event<T: Config> {
        /// Event emitted when the crowdloan claim pallet is properly configured.
        PalletInitialized(),

        /// Event emitted when a reward has been claimed successfully.
        RewardClaimed(
            T::RelayChainAccountId,
            ParachainAccountIdOf<T>,
            ContributionAmountOf<T>,
        ),
    }

    // ------------------------------------------------------------------------
    // Pallet storage items
    // ------------------------------------------------------------------------

    /// List of contributors and their respective contributions.
    ///
    /// This child trie root hash contains the list of contributors and their respective
    /// contributions. Polkadot provides an efficient base-16 modified Merkle Patricia tree
    /// for implementing [`trie`](https://github.com/paritytech/trie) data structure.
    /// This root hash is copied from the relaychain's [`crowdloan`](https://github.com/paritytech/polkadot/blob/rococo-v1/runtime/common/src/crowdloan.rs)
    /// module via the signed [`initialize`] transaction (or extrinsics). It is used to
    /// check if a contributor is elligible for a reward payout.
    #[pallet::storage]
    #[pallet::getter(fn contributions)]
    pub(super) type Contributions<T: Config> = StorageValue<_, ChildTrieRootHashOf<T>, OptionQuery>;

    /// TrieIndex of the crowdloan campaign inside the relay-chain crowdloan module.
    ///
    /// This is needed in order to build the correct keys for proof check.
    #[pallet::storage]
    #[pallet::getter(fn crowdloan_trie_index)]
    pub type CrowdloanTrieIndex<T: Config> = StorageValue<_, TrieIndex>;

    /// A map containing the list of claims for reward payouts that were successfuly processed
    #[pallet::storage]
    #[pallet::getter(fn processed_claims)]
    pub(super) type ProcessedClaims<T: Config> =
        StorageMap<_, Blake2_128Concat, T::RelayChainAccountId, T::BlockNumber>;

    // ----------------------------------------------------------------------------
    // Pallet lifecycle hooks
    // ----------------------------------------------------------------------------

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // ------------------------------------------------------------------------
    // Pallet errors
    // ------------------------------------------------------------------------

    #[pallet::error]
    pub enum Error<T> {
        /// Cannot re-initialize the pallet
        PalletAlreadyInitialized,

        /// Cannot call reward before pallet is initialized
        PalletNotInitialized,

        /// Claim has already been processed (replay attack, probably)
        ClaimAlreadyProcessed,

        /// The proof of a contribution is invalid
        InvalidProofOfContribution,

        /// Claimed amount is out of boundaries (too low or too high)
        ClaimedAmountIsOutOfBoundaries,

        /// Sensitive transactions can only be performed by administrator entity (e.g. Sudo or Democracy pallet)
        MustBeAdministrator,

        /// The reward amount that is claimed does not correspond to the one of the contribution
        InvalidClaimAmount,

        /// The signature provided by the contributor when registering is not valid.
        ///
        /// The consequence is that the relaychain and parachain accounts being not
        /// associated, the contributor is not elligible for a reward payout.
        InvalidContributorSignature,
    }

    // ------------------------------------------------------------------------
    // Pallet dispatchable functions
    // ------------------------------------------------------------------------

    // Declare Call struct and implement dispatchable (or callable) functions.
    //
    // Dispatchable functions are transactions modifying the state of the chain. They
    // are also called extrinsics are constitute the pallet's public interface.
    // Note that each parameter used in functions must implement `Clone`, `Debug`,
    // `Eq`, `PartialEq` and `Codec` traits.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim for a reward payout via an unsigned transaction
        ///
        /// An unsigned transaction is free of fees. We need such an unsigned transaction
        /// as some contributors may not have enought parachain tokens for claiming their
        /// reward payout. The [`validate_unsigned`] function first checks the validity of
        /// this transaction, so that to prevent potential frauds or attacks.
        /// Transactions that call that function are de-duplicated on the pool level
        /// via `validate_unsigned` implementation.
        /// It is worth pointing out that despite unsigned transactions are free of charge,
        /// a weight must be assigned to them so that to prevent a single block of having
        /// infinite number of such transactions.
        /// The [`contributor_identity_proof`] is built using a signature of the contributor's
        /// parachain account id with the claimer key.
        /// See [`validate_unsigned`]
        #[pallet::weight(<T as Config>::WeightInfo::claim_reward())]
        pub(crate) fn claim_reward(
            origin: OriginFor<T>,
            relaychain_account_id: T::RelayChainAccountId,
            parachain_account_id: ParachainAccountIdOf<T>,
            claimed_amount: ContributionAmountOf<T>,
            identity_proof: MultiSignature,
            contribution_proof: Vec<Vec<u8>>,
        ) -> DispatchResultWithPostInfo {
            // Ensures that this function can only be called via an unsigned transaction
            ensure_none(origin)?;

            // Ensure that the module has been initialized before calling this
            ensure!(
                <Contributions<T>>::get() != None,
                Error::<T>::PalletNotInitialized
            );

            // Be sure user has not already claimed her/his reward payout
            ensure!(
                !ProcessedClaims::<T>::contains_key(&relaychain_account_id),
                Error::<T>::ClaimAlreadyProcessed
            );

            // Claimed amount must be positive value
            ensure!(
                !claimed_amount.is_zero(),
                Error::<T>::ClaimedAmountIsOutOfBoundaries
            );

            // Check (trustless) contributor identity
            Self::verify_contributor_identity_proof(
                relaychain_account_id.clone(),
                parachain_account_id.clone(),
                identity_proof,
            )?;

            // Check the contributor's proof of contribution
            Self::verify_contribution_proof(
                relaychain_account_id.clone(),
                claimed_amount,
                contribution_proof,
            )?;

            // Invoke the reward payout mechanism
            T::RewardMechanism::reward(parachain_account_id.clone(), claimed_amount)?;

            // Store this claim in the list of processed claims (so that to process it only once)
            // TODO [TankOfZion]: `Module` must be replaced by `Pallet` when all code base will be ported to FRAME v2
            <ProcessedClaims<T>>::insert(
                &relaychain_account_id,
                <frame_system::Module<T>>::block_number(),
            );

            Self::deposit_event(Event::RewardClaimed(
                relaychain_account_id,
                parachain_account_id,
                claimed_amount,
            ));

            Ok(().into())
        }

        /// Initialize the claim pallet
        ///
        /// This administrative function is used to transfer the list of contributors
        /// and their respective contributions, stored as a child trie root hash in
        /// the relay chain's [`crowdloan`](https://github.com/paritytech/polkadot/blob/rococo-v1/runtime/common/src/crowdloan.rs)
        /// module, to [`Contributions`] storage item.
        /// This transaction can only be called via a signed transactions.
        /// The [`contributions`] parameter contains the hash of the crowdloan pallet's child
        /// trie root. It is later used for proving that a contributor effectively contributed
        /// to the crowdloan campaign, and that the amount of the contribution is correct as
        /// well.
        #[pallet::weight(<T as Config>::WeightInfo::initialize())]
        pub(crate) fn initialize(
            origin: OriginFor<T>,
            contributions: ChildTrieRootHashOf<T>,
            index: TrieIndex,
        ) -> DispatchResultWithPostInfo {
            // Ensure that only administrator entity can perform this administrative transaction
            ensure!(
                Self::ensure_administrator(origin).is_ok(),
                Error::<T>::MustBeAdministrator
            );

            // Ensure that the pallet has not already been initialized
            // TODO: Should we change this? This would basically allow the pallet to be used exactly once, without an RuntimeUpgrade.
            ensure!(
                <Contributions<T>>::get().is_none(),
                Error::<T>::PalletAlreadyInitialized
            );

            // Store relay chain's child trie root hash (containing the list of contributors and their contributions)
            <Contributions<T>>::put(contributions);

            <CrowdloanTrieIndex<T>>::put(index);

            // Trigger an event so that to inform that the pallet was successfully initialized
            Self::deposit_event(Event::PalletInitialized());

            Ok(().into())
        }
    }

    // ------------------------------------------------------------------------
    // Pallet unsigned transactions validation
    // ------------------------------------------------------------------------

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        /// Validate unsigned transactions
        ///
        /// Unsigned transactions are generally disallowed. However, since a contributor
        /// claiming a reward payout may not have the necessary tokens on the parachain to
        /// pay the fees of the claim, the [`claim_reward`] transactions must be
        /// unsigned.
        /// Here, we make sure such unsigned, and remember, feeless unsigned transactions
        /// can be used for malicious spams or Deny of Service (DoS) attacks.
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::claim_reward(
                relaychain_account_id,
                parachain_account_id,
                claimed_amount,
                identity_proof,
                contribution_proof,
            ) = call
            {
                // By checking the validity of the claim here, we ensure the extrinsic will not
                // make it into a block (in case of a trusted node, not even into the pool)
                // unless being valid. This is a trade-off between protecting the network from spam
                // and paying validators for the work they are doing.
                if !ProcessedClaims::<T>::contains_key(&relaychain_account_id)
                    && Self::verify_contributor_identity_proof(
                        relaychain_account_id.clone(),
                        parachain_account_id.clone(),
                        identity_proof.clone(),
                    )
                    .is_ok()
                    && Self::verify_contribution_proof(
                        relaychain_account_id.clone(),
                        claimed_amount.clone(),
                        contribution_proof.to_vec(),
                    )
                    .is_ok()
                {
                    // Only the claim reward transaction can be invoked via an unsigned regime
                    return ValidTransaction::with_tag_prefix("CrowdloanClaimReward")
                        .priority(T::ClaimTransactionPriority::get())
                        .and_provides((relaychain_account_id, parachain_account_id, claimed_amount))
                        .longevity(
                            TryInto::<u64>::try_into(T::ClaimTransactionLongevity::get())
                                .unwrap_or(64_u64),
                        )
                        .propagate(true)
                        .build();
                } else {
                    return InvalidTransaction::BadProof.into();
                }
            }

            // Dissallow other unsigned transactions
            InvalidTransaction::Call.into()
        }
    }
} // end of 'pallet' module

// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Pallet implementation block.
//
// This main implementation block contains two categories of functions, namely:
// - Public functions: These are functions that are `pub` and generally fall into
//   inspector functions that do not write to storage and operation functions that do.
// - Private functions: These are private helpers or utilities that cannot be called
//   from other pallets.
impl<T: Config> Pallet<T> {
    /// Return the account identifier of the crowdloan claim pallet.
    ///
    /// This actually does computation. If you need to keep using it, then make
    /// sure you cache the value and only call this once.
    pub fn account_id() -> T::AccountId {
        T::ModuleId::get().into_account()
    }

    // Check if the origin is an administrator or represents the root.
    fn ensure_administrator(origin: T::Origin) -> DispatchResult {
        T::AdminOrigin::try_origin(origin)
            .map(|_| ())
            .or_else(ensure_root)?;

        Ok(())
    }

    // Bind contributor's relaychain account with one on the parachain.
    //
    // This function aims at proving that the contributor's identity on
    // the relay chain is valid, using a signature. She/he also provides
    // a parachain account on which the reward payout must be transferred
    // and the amount she/he contributed to.
    // The [`signature`] is used as the proof.
    fn verify_contributor_identity_proof(
        relaychain_account_id: T::RelayChainAccountId,
        parachain_account_id: ParachainAccountIdOf<T>,
        signature: MultiSignature,
    ) -> DispatchResult {
        // Now check if the contributor's native identity on the relaychain is valid
        let payload = parachain_account_id.encode();
        ensure!(
            signature.verify(payload.as_slice(), &relaychain_account_id.clone().into()),
            Error::<T>::InvalidContributorSignature
        );

        Ok(())
    }

    // Verify that the contributor is eligible for a reward payout.
    //
    // The [`Contributions`] child trie root hash contains all contributions and their respective
    // contributors. Given the contributor's relay chain account identifier, the claimed amount
    // (in relay chain tokens) and the parachain account identifier, this function proves that the
    // contributor's claim is valid.
    fn verify_contribution_proof(
        relay_account: T::RelayChainAccountId,
        contribution: ContributionAmountOf<T>,
        proof: Vec<Vec<u8>>,
    ) -> DispatchResult {
        let proof = StorageProof::new(proof);
        let keys = vec![relay_account.encode()];

        // rebuild child info
        let mut buf = Vec::new();
        buf.extend_from_slice(b"crowdloan");
        buf.extend_from_slice(
            &Self::crowdloan_trie_index()
                .ok_or(Error::<T>::PalletNotInitialized)?
                .encode()[..],
        );
        let child_info = ChildInfo::new_default(T::Hashing::hash(&buf[..]).as_ref());

        // We could unwrap here, as we check in the calling function if module is initialized (i.e. if contributions is set)
        // but better be safe than sorry...
        let root = Self::contributions().ok_or(Error::<T>::PalletNotInitialized)?;

        read_child_proof_check::<T::Hashing, _>(root, proof, &child_info, keys).map_or(
            Err(Error::<T>::InvalidProofOfContribution.into()),
            |mut key_value_map| {
                key_value_map
                    .get_mut(&relay_account.encode())
                    .map_or(&None, |map_val| map_val)
                    .as_ref()
                    .map_or(
                        Err(Error::<T>::InvalidProofOfContribution.into()),
                        |storage_val| {
                            let (value, _memo): (ContributionAmountOf<T>, Vec<u8>) =
                                Decode::decode::<&[u8]>(&mut storage_val.as_ref())
                                    .unwrap_or((Zero::zero(), Vec::new()));
                            ensure!(
                                value == contribution,
                                Error::<T>::InvalidProofOfContribution
                            );

                            Ok(())
                        },
                    )
            },
        )
    }
}
