use crate::proofs;
use sp_core::{Encode, H256};
use frame_system::ensure_none;
use crate::constants::currency;
use sp_std::{vec::Vec, convert::TryInto};
use sp_runtime::traits::{Hash, SaturatedConversion};
use frame_support::{decl_module, decl_storage, decl_event, decl_error,
    traits::Get,
    ensure, dispatch};
use sp_runtime::{
    ModuleId,
    traits::CheckedSub,
    transaction_validity::{
        TransactionValidity, ValidTransaction, InvalidTransaction, TransactionSource,
        TransactionPriority,
    }
};

const MODULE_ID: ModuleId = ModuleId(*b"ct/claim");
const MIN_PAYOUT: node_primitives::Balance     = 5 * currency::RAD;


pub trait Trait: frame_system::Trait + pallet_balances::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    /// An expected duration of the session.
    ///
    /// This parameter is used to determine the longevity of `heartbeat` transaction
    /// and a rough time when we should start considering sending heartbeats,
    /// since the workers avoids sending them at the very beginning of the session, assuming
    /// there is a chance the authority will produce a block and they won't be necessary.
    type SessionDuration: Get<Self::BlockNumber>;

    /// A configuration for base priority of unsigned transactions.
    ///
    /// This is exposed so that it can be tuned for particular runtime, when
    /// multiple pallets send unsigned transactions.
    type UnsignedPriority: Get<TransactionPriority>;
}

decl_storage! {
    trait Store for Module<T: Trait> as RadClaims {
        /// Total unclaimed rewards for an account.
        AccountBalances get(fn get_account_balance): map hasher(blake2_128_concat) T::AccountId => T::Balance = 0.into();
        /// Map of root hashes that correspond to lists of RAD reward claim amounts per account.
        RootHashes get(fn get_root_hash): map hasher(blake2_128_concat) T::Hash => bool;
        /// Account that is allowed to upload new root hashes.
        UploadAccount get(fn get_upload_account): T::AccountId;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait>{
        /// The combination of account id, amount, and proofs vector in a claim was invalid.
        InvalidProofs,
        /// The payout amount attempting to be claimed is less than the minimum allowed by [MIN_PAYOUT].
        UnderMinPayout,
        /// Amount being claimed is less than the available amount in [AccountBalances].
        InsufficientBalance,
    }
}

decl_event! {
    pub enum Event<T> where
        <T as frame_system::Trait>::AccountId,
        <T as pallet_balances::Trait>::Balance,
    {
        Claimed(AccountId, Balance),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = 195_000_000]
        pub fn claim(origin,
                     account_id: T::AccountId,
                     amount: T::Balance,
                     sorted_hashes: Vec<T::Hash>,
        ) -> dispatch::DispatchResult {
            ensure_none(origin)?;

            let claimed = Self::get_account_balance(&account_id);

            // Payout = amount - claim
            let payout = amount.checked_sub(&claimed)
                .ok_or(Error::<T>::InsufficientBalance)?;

            // Payout must not be less than minimum allowed
            ensure!(payout >= MIN_PAYOUT.saturated_into(),
                    Error::<T>::UnderMinPayout);

            // Set account balance to amount
            AccountBalances::<T>::insert(account_id, amount);

            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Hash a:b if a < b, else b:a. Uses the runtime module's hasher.
    pub fn sorted_hash_of(a: &T::Hash, b: &T::Hash) -> T::Hash {
        let mut h: Vec<u8> = Vec::with_capacity(64);
        if a < b {
            h.extend_from_slice(a.as_ref());
            h.extend_from_slice(b.as_ref());
        } else {
            h.extend_from_slice(b.as_ref());
            h.extend_from_slice(a.as_ref());
        }

        T::Hashing::hash_of(&h).into()
    }
}

impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
    type Call = Call<T>;

    fn validate_unsigned(
        _source: TransactionSource,
        call: &Self::Call,
    ) -> TransactionValidity {
        if let Call::claim(account_id, amount, sorted_hashes) = call {
            // Number of proofs should practically never be >30. Checking this
            // blocks abuse.
            if sorted_hashes.len() > 30 {
                return InvalidTransaction::BadProof.into()
            }

            // Concat account id : amount
            let mut v: Vec<u8> = account_id.encode();
            v.extend(amount.encode());

            // Generate root hash
            let leaf_hash = T::Hashing::hash_of(&v);
            let root_hash = sorted_hashes.iter()
                .fold(leaf_hash, |acc, hash| Self::sorted_hash_of(&acc, hash));

            // Check that root exists in root hash storage
            if !Self::get_root_hash(root_hash) == true {
                return ValidTransaction::with_tag_prefix("RADclaim")
                    .priority(T::UnsignedPriority::get())
                    .longevity(TryInto::<u64>::try_into(
                            T::SessionDuration::get() / 2.into())
                        .unwrap_or(64_u64))
                    .propagate(true)
                    .build()
            } else {
                return InvalidTransaction::BadProof.into();
            }
        }

        InvalidTransaction::Call.into()
    }
}
