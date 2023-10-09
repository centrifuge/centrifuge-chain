use std::fmt::Debug;

use cfg_primitives::{
	AccountId, Address, AuraId, Balance, BlockNumber, CollectionId, Header, Index, ItemId, LoanId,
	Moment, PoolId, Signature, TrancheId,
};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use codec::Codec;
use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use fp_self_contained::{SelfContainedCall, UncheckedExtrinsic};
use frame_support::{
	dispatch::{
		DispatchClass, DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo,
		UnfilteredDispatchable,
	},
	inherent::{InherentData, ProvideInherent},
	traits::{Get, IsType},
	weights::WeightToFee as _,
	Parameter,
};
use frame_system::{ChainContext, RawOrigin};
use pallet_transaction_payment::CurrencyAdapter;
use runtime_common::{
	apis,
	fees::{DealWithFees, WeightToFee},
};
use sp_io::TestExternalities;
use sp_runtime::{
	traits::{AccountIdLookup, Block, Checkable, Dispatchable, Extrinsic, Lookup, Member},
	ApplyExtrinsicResult, DispatchResult, Storage,
};
use sp_timestamp::Timestamp;

use crate::{
	generic::{runtime::Runtime, utils::genesis::Genesis},
	utils::accounts::Keyring,
};

/// Used by Env::pass() to determine how many blocks should be passed
#[derive(Clone)]
pub enum Blocks<T: Runtime> {
	/// Pass X blocks
	ByNumber(BlockNumber),

	/// Pass a number of blocks enough to emulate the given passage of time.
	/// i.e. choosing 1 sec would pass 1 block to emulate such change in the
	/// time.
	BySeconds(Moment),

	/// Pass a number of block until find an event or reach the limit
	UntilEvent {
		event: T::RuntimeEventExt,
		limit: BlockNumber,
	},
}

/// Define an environment behavior
pub trait Env<T: Runtime> {
	/// Load the environment from a storage
	fn from_storage(storage: Storage) -> Self;

	/// Submit an extrinsic mutating the state
	fn submit(&mut self, who: Keyring, call: impl Into<T::RuntimeCall>) -> DispatchResult;

	/// Pass any number of blocks
	fn pass(&mut self, blocks: Blocks<T>);

	/// Allows to mutate the storage state through the closure
	fn state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R;

	/// Allows to read the storage state through the closure
	/// If storage is modified, it would not be applied.
	fn state<R>(&self, f: impl FnOnce() -> R) -> R;

	/// Check for an exact event introduced in the current block.
	/// Starting from last event introduced
	/// Returns an Option to unwrap it from the tests and have good panic
	/// message with the error test line
	fn check_event(&self, event: impl Into<T::RuntimeEventExt>) -> Option<()> {
		self.state(|| {
			let event = event.into();
			frame_system::Pallet::<T>::events()
				.into_iter()
				.rev()
				.find(|record| record.event == event)
				.map(|_| ())
		})
	}

	/// Find an event introduced in the current block
	/// Starting from last event introduced
	/// Returns an Option to unwrap it from the tests and have good panic
	/// message with the error test line
	fn find_event<E, R>(&self, f: impl Fn(E) -> Option<R>) -> Option<R>
	where
		T::RuntimeEventExt: TryInto<E>,
	{
		self.state(|| {
			frame_system::Pallet::<T>::events()
				.into_iter()
				.rev()
				.map(|record| record.event.try_into().ok())
				.find_map(|event| event.map(|e| f(e)))
				.flatten()
		})
	}

	/// Retrieve the fees used in the last submit call
	fn last_fee(&self) -> Balance {
		self.find_event(|e| match e {
			pallet_transaction_payment::Event::TransactionFeePaid { actual_fee, .. } => {
				Some(actual_fee)
			}
			_ => None,
		})
		.expect("Expected transaction")
	}
}
