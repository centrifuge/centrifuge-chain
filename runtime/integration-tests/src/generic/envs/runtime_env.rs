use std::marker::PhantomData;

use cfg_primitives::{
	AccountId, Address, AuraId, Balance, BlockNumber, CollectionId, Header, Index, ItemId, Moment,
	PoolId, TrancheId,
};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use codec::{Codec, Encode};
use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use fp_self_contained::UncheckedExtrinsic;
use frame_support::{
	assert_ok,
	dispatch::{DispatchClass, UnfilteredDispatchable},
	inherent::{InherentData, ProvideInherent},
	traits::{GenesisBuild, Hooks},
	weights::WeightToFee as _,
};
use frame_system::{ChainContext, RawOrigin};
use runtime_common::fees::WeightToFee;
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_core::{sr25519::Public, H256};
use sp_io::TestExternalities;
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{
		Block, Checkable, Dispatchable, Extrinsic, Get, Lookup, SignedExtension, StaticLookup,
		Verify,
	},
	ApplyExtrinsicResult, Digest, DigestItem, MultiSignature,
};
use sp_timestamp::Timestamp;

use crate::{
	generic::{
		env::{Blocks, Config, Env},
		utils::genesis::Genesis,
	},
	utils::accounts::Keyring,
};

pub struct RuntimeEnv<T: Config> {
	nonce: Index,
	ext: sp_io::TestExternalities,
	_config: PhantomData<T>,
}

impl<T: Config> Env<T> for RuntimeEnv<T> {
	fn from_genesis(builder: Genesis) -> Self {
		let mut ext = sp_io::TestExternalities::new(builder.storage());

		ext.execute_with(|| Self::prepare_block(1));

		Self {
			nonce: 0,
			ext,
			_config: PhantomData,
		}
	}

	fn submit(&mut self, who: Keyring, call: impl Into<T::RuntimeCall>) -> ApplyExtrinsicResult {
		self.ext.execute_with(|| {
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

			T::apply_extrinsic(extrinsic)
		})
	}

	fn pass(&mut self, blocks: Blocks) {
		self.ext.execute_with(|| {
			let next = frame_system::Pallet::<T>::block_number() + 1;

			let last_block = match blocks {
				Blocks::ByNumber(n) => next + n,
				Blocks::BySeconds(secs) => {
					let blocks = secs / pallet_aura::Pallet::<T>::slot_duration();
					if blocks % pallet_aura::Pallet::<T>::slot_duration() != 0 {
						blocks as BlockNumber + 1
					} else {
						blocks as BlockNumber
					}
				}
			};

			for i in next..last_block {
				T::finalize_block();
				Self::prepare_block(i);
			}
		})
	}

	fn state(&mut self, f: impl FnOnce()) {
		self.ext.execute_with(f)
	}
}

impl<T: Config> RuntimeEnv<T> {
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
			parent_hash: [69u8; 32].into(),
		};

		T::initialize_block(&header);

		let timestamp = i as u64 * pallet_aura::Pallet::<T>::slot_duration();
		let inherent_extrinsics = vec![
			Extrinsic::new(Self::cumulus_inherent(), None).unwrap(),
			Extrinsic::new(Self::timestamp_inherent(timestamp), None).unwrap(),
		];

		for extrinsic in inherent_extrinsics {
			T::apply_extrinsic(extrinsic).unwrap();
		}
	}

	fn cumulus_inherent() -> T::RuntimeCall {
		let mut inherent_data = InherentData::default();

		// Cumulus inherent
		let sproof_builder = RelayStateSproofBuilder::default();
		let (relay_parent_storage_root, relay_chain_state) =
			sproof_builder.into_state_root_and_proof();

		let cumulus_inherent = ParachainInherentData {
			validation_data: PersistedValidationData {
				parent_head: vec![].into(),
				relay_parent_number: Default::default(),
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
