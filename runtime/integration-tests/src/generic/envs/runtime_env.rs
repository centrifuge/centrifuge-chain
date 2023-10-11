use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use cfg_primitives::{AuraId, BlockNumber, Header, Index};
use codec::Encode;
use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use frame_support::{
	inherent::{InherentData, ProvideInherent},
	storage::transactional,
	traits::GenesisBuild,
};
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_core::{sr25519::Public, H256};
use sp_runtime::{
	traits::{Block, Extrinsic},
	Digest, DigestItem, DispatchError, DispatchResult, Storage, TransactionOutcome,
};
use sp_timestamp::Timestamp;

use crate::generic::{environment::Env, runtime::Runtime};

/// Evironment that interact directly with the runtime,
/// without the usage of a client.
pub struct RuntimeEnv<T: Runtime> {
	nonce: Index,
	ext: Rc<RefCell<sp_io::TestExternalities>>,
	_config: PhantomData<T>,
}

impl<T: Runtime> Env<T> for RuntimeEnv<T> {
	fn from_storage(mut storage: Storage) -> Self {
		// Needed for the aura usage
		pallet_aura::GenesisConfig::<T> {
			authorities: vec![AuraId::from(Public([0; 32]))],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(storage);

		ext.execute_with(|| Self::prepare_block(1));

		Self {
			nonce: 0,
			ext: Rc::new(RefCell::new(ext)),
			_config: PhantomData,
		}
	}

	fn state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.ext.borrow_mut().execute_with(f)
	}

	fn state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.ext.borrow_mut().execute_with(|| {
			transactional::with_transaction(|| {
				// We revert all changes done by the closure to offer an inmutable state method
				TransactionOutcome::Rollback::<Result<R, DispatchError>>(Ok(f()))
			})
			.unwrap()
		})
	}

	fn __priv_build_block(&mut self, i: BlockNumber) {
		self.state_mut(|| {
			T::finalize_block();
			Self::prepare_block(i);
		});
	}

	fn __priv_apply_extrinsic(
		&mut self,
		extrinsic: <T::Block as Block>::Extrinsic,
	) -> DispatchResult {
		self.state_mut(|| T::apply_extrinsic(extrinsic).unwrap())
	}
}

impl<T: Runtime> RuntimeEnv<T> {
	fn prepare_block(i: BlockNumber) {
		let slot = Slot::from(i as u64);
		let digest = Digest {
			logs: vec![DigestItem::PreRuntime(AURA_ENGINE_ID, slot.encode())],
		};

		let header = Header {
			number: i,
			digest,
			state_root: H256::default(),
			extrinsics_root: H256::default(),
			parent_hash: H256::default(),
		};

		T::initialize_block(&header);

		let timestamp = i as u64 * pallet_aura::Pallet::<T>::slot_duration();
		let inherent_extrinsics = vec![
			Extrinsic::new(Self::cumulus_inherent(i), None).unwrap(),
			Extrinsic::new(Self::timestamp_inherent(timestamp), None).unwrap(),
		];

		for extrinsic in inherent_extrinsics {
			T::apply_extrinsic(extrinsic).unwrap().unwrap();
		}
	}

	fn cumulus_inherent(i: BlockNumber) -> T::RuntimeCall {
		let mut inherent_data = InherentData::default();

		let sproof_builder = RelayStateSproofBuilder::default();
		let (relay_parent_storage_root, relay_chain_state) =
			sproof_builder.into_state_root_and_proof();

		let cumulus_inherent = ParachainInherentData {
			validation_data: PersistedValidationData {
				parent_head: vec![].into(),
				relay_parent_number: i,
				max_pov_size: Default::default(),
				relay_parent_storage_root,
			},
			relay_chain_state,
			downward_messages: Default::default(),
			horizontal_messages: Default::default(),
		};

		inherent_data
			.put_data(
				cumulus_primitives_parachain_inherent::INHERENT_IDENTIFIER,
				&cumulus_inherent,
			)
			.unwrap();

		cumulus_pallet_parachain_system::Pallet::<T>::create_inherent(&inherent_data)
			.unwrap()
			.into()
	}

	fn timestamp_inherent(timestamp: u64) -> T::RuntimeCall {
		let mut inherent_data = InherentData::default();

		let timestamp_inherent = Timestamp::new(timestamp);

		inherent_data
			.put_data(sp_timestamp::INHERENT_IDENTIFIER, &timestamp_inherent)
			.unwrap();

		pallet_timestamp::Pallet::<T>::create_inherent(&inherent_data)
			.unwrap()
			.into()
	}
}
