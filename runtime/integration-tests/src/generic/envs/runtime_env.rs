use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use cfg_primitives::{Address, BlockNumber, Header, Index};
use codec::Encode;
use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use frame_support::{
	inherent::{InherentData, ProvideInherent},
	storage::transactional,
};
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_core::H256;
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{Block, Extrinsic},
	Digest, DigestItem, DispatchError, DispatchResult, MultiSignature, Storage, TransactionOutcome,
};
use sp_timestamp::Timestamp;

use crate::{
	generic::{environment::Env, runtime::Runtime},
	utils::accounts::Keyring,
};

/// Evironment that interact directly with the runtime,
/// without the usage of a client.
pub struct RuntimeEnv<T: Runtime> {
	nonce: Index,
	ext: Rc<RefCell<sp_io::TestExternalities>>,
	_config: PhantomData<T>,
}

impl<T: Runtime> Env<T> for RuntimeEnv<T> {
	fn from_storage(storage: Storage) -> Self {
		let mut ext = sp_io::TestExternalities::new(storage);

		ext.execute_with(|| Self::prepare_block(1));

		Self {
			nonce: 0,
			ext: Rc::new(RefCell::new(ext)),
			_config: PhantomData,
		}
	}

	fn submit(&mut self, who: Keyring, call: impl Into<T::RuntimeCall>) -> DispatchResult {
		self.ext.borrow_mut().execute_with(|| {
			let runtime_call = call.into();
			let signed_extra = (
				frame_system::CheckNonZeroSender::<T>::new(),
				frame_system::CheckSpecVersion::<T>::new(),
				frame_system::CheckTxVersion::<T>::new(),
				frame_system::CheckGenesis::<T>::new(),
				frame_system::CheckEra::<T>::from(Era::mortal(256, 0)),
				frame_system::CheckNonce::<T>::from(self.nonce),
				frame_system::CheckWeight::<T>::new(),
				pallet_transaction_payment::ChargeTransactionPayment::<T>::from(0),
			);

			let raw_payload =
				SignedPayload::new(runtime_call.clone(), signed_extra.clone()).unwrap();
			let signature =
				MultiSignature::Sr25519(raw_payload.using_encoded(|payload| who.sign(payload)));

			let multi_address = (Address::Id(who.to_account_id()), signature, signed_extra);

			let extrinsic =
				<T::Block as Block>::Extrinsic::new(runtime_call, Some(multi_address)).unwrap();

			self.nonce += 1;

			T::apply_extrinsic(extrinsic).unwrap()
		})
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
