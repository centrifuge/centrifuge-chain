use std::{cell::RefCell, marker::PhantomData, mem, rc::Rc};

use cfg_primitives::{AuraId, Balance, BlockNumber, Header};
use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use frame_support::{
	dispatch::GetDispatchInfo,
	inherent::{InherentData, ProvideInherent},
	storage::{transactional, TransactionOutcome},
	traits::BuildGenesisConfig,
};
use frame_system::LastRuntimeUpgradeInfo;
use parity_scale_codec::Encode;
use sp_api::runtime_decl_for_core::CoreV4;
use sp_block_builder::runtime_decl_for_block_builder::BlockBuilderV6;
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_core::{sr25519::Public, Get, H256};
use sp_runtime::{
	traits::Extrinsic,
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	Digest, DigestItem, DispatchError, Storage,
};
use sp_timestamp::Timestamp;

use crate::{
	generic::{
		config::Runtime,
		env::{utils, Env, EnvEvmExtension},
		envs::evm_env::EvmEnv,
	},
	utils::accounts::Keyring,
};

/// Environment that interact directly with the runtime,
/// without the usage of a client.
pub struct RuntimeEnv<T: Runtime> {
	parachain_ext: Rc<RefCell<sp_io::TestExternalities>>,
	sibling_ext: Rc<RefCell<sp_io::TestExternalities>>,
	pending_extrinsics: Vec<(Keyring, T::RuntimeCallExt)>,
	evm: Rc<RefCell<EvmEnv<T>>>,
	_config: PhantomData<T>,
}

impl<T: Runtime> Default for RuntimeEnv<T> {
	fn default() -> Self {
		Self::from_storage(Default::default(), Default::default(), Default::default())
	}
}

impl<T: Runtime> EnvEvmExtension<T> for RuntimeEnv<T> {
	type EvmEnv = EvmEnv<T>;

	fn state_mut<R>(&mut self, f: impl FnOnce(&mut Self::EvmEnv) -> R) -> R {
		self.parachain_ext
			.borrow_mut()
			.execute_with(|| f(&mut *self.evm.borrow_mut()))
	}

	fn state<R>(&self, f: impl FnOnce(&Self::EvmEnv) -> R) -> R {
		self.parachain_ext
			.borrow_mut()
			.execute_with(|| f(&*self.evm.borrow()))
	}
}

impl<T: Runtime> Env<T> for RuntimeEnv<T> {
	fn from_parachain_storage(parachain_storage: Storage) -> Self {
		Self::from_storage(Default::default(), parachain_storage, Default::default())
	}

	fn from_storage(
		_relay_storage: Storage,
		parachain_storage: Storage,
		sibling_storage: Storage,
	) -> Self {
		Self {
			parachain_ext: Self::build_externality(parachain_storage),
			sibling_ext: Self::build_externality(sibling_storage),
			pending_extrinsics: Vec::default(),
			evm: Rc::new(RefCell::new(EvmEnv::default())),
			_config: PhantomData,
		}
	}

	fn submit_now(
		&mut self,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) -> Result<Balance, DispatchError> {
		let call: T::RuntimeCallExt = call.into();
		let info = self.parachain_state(|| call.get_dispatch_info());

		let extrinsic = self.parachain_state(|| {
			let nonce = frame_system::Pallet::<T>::account(who.id()).nonce;
			utils::create_extrinsic::<T>(who, call, nonce)
		});
		let len = extrinsic.encoded_size();

		self.parachain_state_mut(|| {
			let res = T::Api::apply_extrinsic(extrinsic);
			// NOTE: This is our custom error that we are having in the
			//       `PreBalanceTransferExtension` SignedExtension, so we need to
			//        catch that here.
			if let Err(TransactionValidityError::Invalid(InvalidTransaction::Custom(255))) = res {
				Ok(Ok(()))
			} else {
				res
			}
			.unwrap()
		})?;

		let fee = self
			.find_event(|e| match e {
				pallet_transaction_payment::Event::TransactionFeePaid { actual_fee, .. } => {
					Some(actual_fee)
				}
				_ => None,
			})
			.unwrap_or_else(|| {
				self.parachain_state(|| {
					pallet_transaction_payment::Pallet::<T>::compute_fee(len as u32, &info, 0)
				})
			});

		Ok(fee)
	}

	fn submit_later(
		&mut self,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) -> Result<(), Box<dyn std::error::Error>> {
		self.pending_extrinsics.push((who, call.into()));
		Ok(())
	}

	fn relay_state_mut<R>(&mut self, _f: impl FnOnce() -> R) -> R {
		unimplemented!("Mutable relay state not implemented for RuntimeEnv")
	}

	fn relay_state<R>(&self, _f: impl FnOnce() -> R) -> R {
		unimplemented!("Relay state not implemented for RuntimeEnv")
	}

	fn parachain_state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.parachain_ext.borrow_mut().execute_with(f)
	}

	fn parachain_state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.parachain_ext.borrow_mut().execute_with(|| {
			transactional::with_transaction(|| {
				let result = f();

				// We do not want to apply any changes, because this is inmutable
				// only check if there is no error in applying it.
				TransactionOutcome::Rollback(Ok::<_, DispatchError>(result))
			})
			.expect("Rollback result is always Ok")
		})
	}

	fn sibling_state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.sibling_ext.borrow_mut().execute_with(f)
	}

	fn sibling_state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.sibling_ext.borrow_mut().execute_with(|| {
			transactional::with_transaction(|| {
				let result = f();

				// We do not want to apply any changes, because this is inmutable
				// only check if there is no error in applying it.
				TransactionOutcome::Rollback(Ok::<_, DispatchError>(result))
			})
			.expect("Rollback result is always Ok")
		})
	}

	fn __priv_build_block(&mut self, i: BlockNumber) {
		self.process_pending_extrinsics();
		self.parachain_state_mut(|| {
			T::Api::finalize_block();
			Self::prepare_block(i);
		});
	}
}

impl<T: Runtime> RuntimeEnv<T> {
	fn build_externality(storage: Storage) -> Rc<RefCell<sp_io::TestExternalities>> {
		let mut ext = sp_io::TestExternalities::new(storage);

		ext.execute_with(|| {
			// Precompiles need to have code-set
			pallet_evm::GenesisConfig::<T> {
				accounts: runtime_common::evm::precompile::utils::precompile_account_genesis::<
					T::PrecompilesTypeExt,
				>(),
				_marker: PhantomData::default(),
			}
			.build();

			// NOTE: Setting the current on-chain runtime version to the latest one, to
			//       prevent running migrations
			frame_system::LastRuntimeUpgrade::<T>::put(LastRuntimeUpgradeInfo::from(
				<T as frame_system::Config>::Version::get(),
			));

			// If no authorities we set a default authority
			if pallet_aura::Pallet::<T>::authorities().is_empty() {
				pallet_aura::Pallet::<T>::initialize_authorities(&[AuraId::from(Public([0; 32]))]);
			}

			Self::prepare_block(1);
		});

		Rc::new(RefCell::new(ext))
	}

	fn process_pending_extrinsics(&mut self) {
		let pending_extrinsics = mem::replace(&mut self.pending_extrinsics, Vec::default());

		for (who, call) in pending_extrinsics {
			let extrinsic = self.parachain_state(|| {
				let nonce = frame_system::Pallet::<T>::account(who.id()).nonce;
				utils::create_extrinsic::<T>(who, call, nonce)
			});

			self.parachain_state_mut(|| T::Api::apply_extrinsic(extrinsic).unwrap().unwrap());
		}
	}

	pub fn prepare_block(i: BlockNumber) {
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

		T::Api::initialize_block(&header);

		let timestamp = i as u64 * pallet_aura::Pallet::<T>::slot_duration();
		let inherent_extrinsics = vec![
			Extrinsic::new(Self::cumulus_inherent(i), None).unwrap(),
			Extrinsic::new(Self::timestamp_inherent(timestamp), None).unwrap(),
		];

		for extrinsic in inherent_extrinsics {
			T::Api::apply_extrinsic(extrinsic).unwrap().unwrap();
		}
	}

	fn cumulus_inherent(i: BlockNumber) -> T::RuntimeCallExt {
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

	fn timestamp_inherent(timestamp: u64) -> T::RuntimeCallExt {
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

#[cfg(test)]
mod tests {
	use cfg_primitives::CFG;

	use super::*;
	use crate::generic::{env::Blocks, utils::genesis::Genesis};

	#[test_runtimes(all)]
	fn correct_nonce_for_submit_now<T: Runtime>() {
		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(pallet_balances::GenesisConfig::<T> {
					balances: vec![(Keyring::Alice.id(), 1 * CFG)],
				})
				.storage(),
		);

		env.submit_now(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.submit_now(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();
	}

	#[test_runtimes(all)]
	fn correct_nonce_for_submit_later<T: Runtime>() {
		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(pallet_balances::GenesisConfig::<T> {
					balances: vec![(Keyring::Alice.id(), 1 * CFG)],
				})
				.storage(),
		);

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.pass(Blocks::ByNumber(1));

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();
	}
}
