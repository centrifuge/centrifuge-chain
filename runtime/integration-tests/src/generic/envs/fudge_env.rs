pub mod handle;

use cfg_primitives::{Address, BlockNumber};
use codec::Encode;
use fudge::primitives::Chain;
use handle::{FudgeHandle, ParachainClient};
use sc_client_api::HeaderBackend;
use sp_api::{ApiRef, ProvideRuntimeApi};
use sp_runtime::{
	generic::{BlockId, Era, SignedPayload},
	traits::{Block, Extrinsic},
	DispatchResult, MultiSignature, Storage,
};

use crate::{
	generic::{environment::Env, runtime::Runtime},
	utils::accounts::Keyring,
};

/// Trait that represent a runtime with Fudge support
pub trait FudgeSupport: Runtime {
	/// Type to interact with fudge
	type FudgeHandle: FudgeHandle<Self>;
}

/// Evironment that uses fudge to interact with the runtime
pub struct FudgeEnv<T: Runtime + FudgeSupport> {
	handle: T::FudgeHandle,
}

impl<T: Runtime + FudgeSupport> Env<T> for FudgeEnv<T> {
	fn from_storage(storage: Storage) -> Self {
		Self {
			handle: T::FudgeHandle::build(Storage::default(), storage),
		}
	}

	fn submit(&mut self, who: Keyring, call: impl Into<T::RuntimeCall>) -> DispatchResult {
		let runtime_call = call.into();
		let signed_extra = (
			frame_system::CheckNonZeroSender::<T>::new(),
			frame_system::CheckSpecVersion::<T>::new(),
			frame_system::CheckTxVersion::<T>::new(),
			frame_system::CheckGenesis::<T>::new(),
			frame_system::CheckEra::<T>::from(Era::mortal(256, 0)),
			frame_system::CheckNonce::<T>::from(0),
			frame_system::CheckWeight::<T>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<T>::from(0),
		);

		let raw_payload = SignedPayload::new(runtime_call.clone(), signed_extra.clone()).unwrap();
		let signature =
			MultiSignature::Sr25519(raw_payload.using_encoded(|payload| who.sign(payload)));

		let multi_address = (Address::Id(who.to_account_id()), signature, signed_extra);

		let extrinsic =
			<T::Block as Block>::Extrinsic::new(runtime_call, Some(multi_address)).unwrap();

		self.handle
			.parachain_mut()
			.append_extrinsic(extrinsic)
			.unwrap();

		Ok(())
	}

	fn state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.handle.parachain_mut().with_mut_state(f).unwrap()
	}

	fn state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.handle.parachain().with_state(f).unwrap()
	}

	fn __priv_build_block(&mut self, _i: BlockNumber) {
		self.handle.evolve();
	}
}

type ApiRefOf<'a, T> = ApiRef<
	'a,
	<ParachainClient<
		<T as Runtime>::Block,
		<<T as FudgeSupport>::FudgeHandle as FudgeHandle<T>>::ParachainConstructApi,
	> as sp_api::ProvideRuntimeApi<<T as Runtime>::Block>>::Api,
>;

/// Specialized fudge methods
impl<T: Runtime + FudgeSupport> FudgeEnv<T> {
	pub fn chain_state_mut<R>(&mut self, chain: Chain, f: impl FnOnce() -> R) -> R {
		self.handle.with_mut_state(chain, f)
	}

	pub fn chain_state<R>(&self, chain: Chain, f: impl FnOnce() -> R) -> R {
		self.handle.with_state(chain, f)
	}

	pub fn with_api<F>(&self, exec: F)
	where
		F: FnOnce(ApiRefOf<T>, BlockId<T::Block>),
	{
		let client = self.handle.parachain().client();
		let best_hash = client.info().best_hash;
		let api = client.runtime_api();
		let best_hash = BlockId::hash(best_hash);

		exec(api, best_hash);
	}
}
