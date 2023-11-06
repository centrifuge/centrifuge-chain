use cfg_primitives::{currency_decimals, parachains, AccountId, Balance};
use cfg_types::{
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use cfg_utils::vec_to_fixed_array;
use codec::Encode;
use frame_support::{assert_noop, assert_ok, dispatch::RawOrigin, traits::OriginTrait};
use orml_traits::{asset_registry::AssetMetadata, MultiCurrency};
use polkadot_parachain::primitives::Id;
use runtime_common::{
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert as C2},
	WeakBoundedVec,
};
use xcm::{
	prelude::XCM_VERSION,
	v3::{
		AssetId, Fungibility, Instruction::WithdrawAsset, Junction, Junction::*, Junctions,
		Junctions::*, MultiAsset, MultiAssets, MultiLocation, NetworkId, WeightLimit, Xcm,
	},
	VersionedMultiAsset, VersionedMultiAssets, VersionedMultiLocation,
};
use xcm_executor::traits::Convert as C1;

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeSupport},
		utils::{genesis, genesis::Genesis},
	},
	utils::{accounts::Keyring, AUSD_CURRENCY_ID},
};

mod utils {
	use super::*;

	pub(crate) fn parachain_account(id: u32) -> AccountId {
		polkadot_parachain::primitives::Sibling::from(id).into_account_truncating()
	}
}

type FudgeRelayRuntime<T> = <<T as FudgeSupport>::FudgeHandle as FudgeHandle<T>>::RelayRuntime;

use utils::*;

mod altair {
	use altair_runtime::{CurrencyIdConvert, PoolPalletIndex};

	pub const KSM_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(1000);

	use super::*;

	mod utils {
		use super::*;

		pub fn setup_xcm<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			env.parachain_state_mut(|| {
				// Set the XCM version used when sending XCM messages to sibling.
				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(
						1,
						Junctions::X1(Junction::Parachain(T::FudgeHandle::SIBLING_ID)),
					)),
					XCM_VERSION,
				));
			});

			env.sibling_state_mut(|| {
				// Set the XCM version used when sending XCM messages to sibling.
				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(
						1,
						Junctions::X1(Junction::Parachain(T::FudgeHandle::PARA_ID)),
					)),
					XCM_VERSION,
				));
			});

			env.relay_state_mut(|| {
				assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
					FudgeRelayRuntime<T>,
				>::force_open_hrmp_channel(
					<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
					Id::from(T::FudgeHandle::PARA_ID),
					Id::from(T::FudgeHandle::SIBLING_ID),
					10,
					1024,
				));

				assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
					FudgeRelayRuntime<T>,
				>::force_open_hrmp_channel(
					<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
					Id::from(T::FudgeHandle::SIBLING_ID),
					Id::from(T::FudgeHandle::PARA_ID),
					10,
					1024,
				));

				assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
					FudgeRelayRuntime<T>,
				>::force_process_hrmp_open(
					<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
					0,
				));
			});

			env.pass(Blocks::ByNumber(1));
		}

		pub fn register_ausd<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Acala Dollar".into(),
				symbol: "AUSD".into(),
				existential_deposit: 1_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::SIBLING_ID),
						general_key(parachains::kusama::karura::AUSD_KEY),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(AUSD_CURRENCY_ID)
			));
		}

		pub fn register_air<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Altair".into(),
				symbol: "AIR".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(parachains::kusama::altair::ID),
						general_key(parachains::kusama::altair::AIR_KEY),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(CurrencyId::Native)
			));
		}

		pub fn register_ksm<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Kusama".into(),
				symbol: "KSM".into(),
				existential_deposit: 1_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(1, Here))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(KSM_ASSET_ID)
			));
		}

		pub fn air(amount: Balance) -> Balance {
			amount * dollar(currency_decimals::NATIVE)
		}

		pub fn ausd(amount: Balance) -> Balance {
			amount * dollar(currency_decimals::AUSD)
		}

		pub fn ksm(amount: Balance) -> Balance {
			amount * dollar(currency_decimals::KSM)
		}

		pub fn foreign(amount: Balance, decimals: u32) -> Balance {
			amount * dollar(decimals)
		}

		pub fn dollar(decimals: u32) -> Balance {
			10u128.saturating_pow(decimals)
		}

		pub fn air_fee() -> Balance {
			fee(currency_decimals::NATIVE)
		}

		pub fn ausd_fee() -> Balance {
			fee(currency_decimals::AUSD)
		}

		pub fn fee(decimals: u32) -> Balance {
			calc_fee(default_per_second(decimals))
		}

		// The fee associated with transferring KSM tokens
		pub fn ksm_fee() -> Balance {
			calc_fee(ksm_per_second())
		}

		pub fn calc_fee(fee_per_second: Balance) -> Balance {
			// We divide the fee to align its unit and multiply by 4 as that seems to be the
			// unit of time the tests take.
			// NOTE: it is possible that in different machines this value may differ. We
			// shall see.
			fee_per_second.div_euclid(10_000) * 8
		}

		pub fn xcm_metadata(transferability: CrossChainTransferability) -> Option<XcmMetadata> {
			match transferability {
				CrossChainTransferability::Xcm(x) | CrossChainTransferability::All(x) => Some(x),
				_ => None,
			}
		}
	}

	use utils::*;

	mod restricted_calls {
		use super::*;

		fn xtokens_transfer<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				assert_noop!(
					orml_xtokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						CurrencyId::Tranche(401, [0; 16]),
						42,
						Box::new(
							MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										network: None,
										id: Keyring::Bob.into(),
									}
								)
							)
							.into()
						),
						WeightLimit::Limited(8_000_000_000_000.into()),
					),
					orml_xtokens::Error::<T>::NotCrossChainTransferableCurrency
				);
			});
		}

		fn xtokens_transfer_multiasset<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
			let tranche_id =
				WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
			let tranche_location = MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(123),
					PalletInstance(42),
					GeneralKey {
						length: tranche_id.len() as u8,
						data: vec_to_fixed_array(tranche_id.to_vec()),
					},
				),
			};
			let tranche_multi_asset = VersionedMultiAsset::from(MultiAsset::from((
				AssetId::Concrete(tranche_location),
				Fungibility::Fungible(42),
			)));

			env.parachain_state_mut(|| {
				assert_noop!(
					orml_xtokens::Pallet::<T>::transfer_multiasset(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Box::new(tranche_multi_asset),
						Box::new(
							MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										network: None,
										id: Keyring::Bob.into(),
									}
								)
							)
							.into()
						),
						WeightLimit::Limited(8_000_000_000_000.into()),
					),
					orml_xtokens::Error::<T>::XcmExecutionFailed
				);
			});
		}

		fn xtokens_transfer_multiassets<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
			let tranche_id =
				WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
			let tranche_location = MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(123),
					PalletInstance(42),
					GeneralKey {
						length: tranche_id.len() as u8,
						data: vec_to_fixed_array(tranche_id.to_vec()),
					},
				),
			};
			let tranche_multi_asset = MultiAsset::from((
				AssetId::Concrete(tranche_location),
				Fungibility::Fungible(42),
			));

			env.parachain_state_mut(|| {
				assert_noop!(
					orml_xtokens::Pallet::<T>::transfer_multiassets(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Box::new(VersionedMultiAssets::from(MultiAssets::from(vec![
							tranche_multi_asset
						]))),
						0,
						Box::new(
							MultiLocation::new(
								1,
								X2(
									Parachain(T::FudgeHandle::SIBLING_ID),
									Junction::AccountId32 {
										network: None,
										id: Keyring::Bob.into(),
									}
								)
							)
							.into()
						),
						WeightLimit::Limited(8_000_000_000_000.into()),
					),
					orml_xtokens::Error::<T>::XcmExecutionFailed
				);
			});
		}

		crate::test_for_runtimes!([altair], xtokens_transfer);
		crate::test_for_runtimes!([altair], xtokens_transfer_multiasset);
		crate::test_for_runtimes!([altair], xtokens_transfer_multiassets);
	}

	mod transfers {
		use super::*;

		fn transfer_air_to_sibling<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_balance = air(10);
			let bob_initial_balance = air(10);
			let transfer_amount = air(5);
			let air_in_sibling = CurrencyId::ForeignAsset(12);

			env.parachain_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance
				);
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					0
				);

				// Register AIR as foreign asset in the sibling parachain
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Altair".into(),
					symbol: "AIR".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(
							Parachain(T::FudgeHandle::PARA_ID),
							general_key(parachains::kusama::altair::AIR_KEY),
						),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::Native)
				));
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into()),
					0
				);

				// Register AIR as foreign asset in the sibling parachain
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Altair".into(),
					symbol: "AIR".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(
							Parachain(T::FudgeHandle::PARA_ID),
							general_key(parachains::kusama::altair::AIR_KEY),
						),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(air_in_sibling)
				));
			});

			env.pass(Blocks::ByNumber(1));

			env.parachain_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					CurrencyId::Native,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::SIBLING_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance - transfer_amount
				);

				// Verify that the amount transferred is now part of the sibling account here
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::SIBLING_ID
					)),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.sibling_state_mut(|| {
				let current_balance =
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has (amount transferred - fee)
				assert_eq!(current_balance, transfer_amount - fee(18));

				// Sanity check for the actual amount Keyring::Bob ends up with
				assert_eq!(current_balance, 4992960800000000000);
			});
		}

		fn test_air_transfers_to_and_from_sibling<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Genesis::default()
					.add(genesis::balances::<T>(air(10)))
					.storage(),
				Default::default(),
			);

			setup_xcm(&mut env);

			// In order to be able to transfer AIR from Sibling to Altair, we need to first
			// send AIR from Altair to Sibling, or else it fails since it'd be like Sibling
			// had minted AIR on their side.
			transfer_air_to_sibling(&mut env);

			let alice_initial_balance = air(5);
			let bob_initial_balance = air(5) - air_fee();
			let transfer_amount = air(1);

			// Note: This asset was registered in `transfer_air_to_sibling`
			let air_in_sibling = CurrencyId::ForeignAsset(12);

			env.parachain_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance
				);
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&parachain_account(
						T::FudgeHandle::PARA_ID
					)),
					0
				);
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance
				);

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					air_in_sibling,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Alice.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Bobs's balance is initial balance - amount transferred
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(air_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				// Verify that Keyring::Alice now has initial balance + amount transferred - fee
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance + transfer_amount - air_fee(),
				);
			});
		}

		fn transfer_ausd_to_altair<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			setup_xcm(&mut env);

			let alice_initial_balance = ausd(10);
			let transfer_amount = ausd(7);

			env.sibling_state_mut(|| {
				register_ausd::<T>();

				assert_ok!(orml_tokens::Pallet::<T>::deposit(
					AUSD_CURRENCY_ID,
					&Keyring::Alice.into(),
					alice_initial_balance
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&parachain_account(T::FudgeHandle::PARA_ID)
					),
					0
				);
			});

			env.parachain_state_mut(|| {
				register_ausd::<T>();

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(AUSD_CURRENCY_ID, &Keyring::Bob.into()),
					0,
				);
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&Keyring::Alice.into()
					),
					ausd(10),
				);
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					AUSD_CURRENCY_ID,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&Keyring::Alice.into()
					),
					alice_initial_balance - transfer_amount
				);

				// Verify that the amount transferred is now part of the altair parachain
				// account here
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(
						AUSD_CURRENCY_ID,
						&parachain_account(T::FudgeHandle::PARA_ID)
					),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				// Verify that Keyring::Bob now has initial balance + amount transferred - fee
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(AUSD_CURRENCY_ID, &Keyring::Bob.into()),
					transfer_amount - ausd_fee()
				);
			});
		}

		fn transfer_ksm_from_relay_chain<T: Runtime + FudgeSupport>(
			env: &mut FudgeEnv<T>,
			transfer_amount: Balance,
			currency_id: CurrencyId,
			meta: AssetMetadata<Balance, CustomMetadata>,
		) {
			env.parachain_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(currency_id),
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &Keyring::Bob.into()),
					0
				);
			});

			env.relay_state_mut(|| {
				assert_ok!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::force_set_balance(
						<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
						Keyring::Alice.to_account_id().into(),
						transfer_amount * 2,
					)
				);

				assert_ok!(
					pallet_xcm::Pallet::<FudgeRelayRuntime<T>>::force_xcm_version(
						<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
						Box::new(MultiLocation::new(
							0,
							Junctions::X1(Junction::Parachain(T::FudgeHandle::PARA_ID)),
						)),
						XCM_VERSION,
					)
				);

				assert_ok!(
					pallet_xcm::Pallet::<FudgeRelayRuntime<T>>::reserve_transfer_assets(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Box::new(Parachain(T::FudgeHandle::PARA_ID).into()),
						Box::new(
							Junction::AccountId32 {
								network: None,
								id: Keyring::Bob.into(),
							}
							.into()
						),
						Box::new((Here, transfer_amount).into()),
						0
					)
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.parachain_state(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &Keyring::Bob.into()),
					transfer_amount - fee(meta.decimals)
				);

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &Keyring::Bob.into()),
					transfer_amount - fee(meta.decimals)
				);
			});
		}

		fn transfer_ksm_to_and_from_relay_chain<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let transfer_amount: Balance = ksm(2);
			let currency_id = CurrencyId::ForeignAsset(3001);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Kusama".into(),
				symbol: "KSM".into(),
				existential_deposit: 1_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(1, Here))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

			// First we need some KSM on Altair
			transfer_ksm_from_relay_chain(&mut env, transfer_amount, currency_id, meta.clone());

			let currency_id = CurrencyId::ForeignAsset(3001);

			env.parachain_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(currency_id, &Keyring::Bob.into()),
					transfer_amount - fee(meta.decimals)
				);

				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(1, Junctions::Here)),
					XCM_VERSION,
				));

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					currency_id,
					ksm(1),
					Box::new(
						MultiLocation::new(
							1,
							X1(Junction::AccountId32 {
								id: Keyring::Bob.into(),
								network: None,
							})
						)
						.into()
					),
					WeightLimit::Limited(4_000_000_000.into())
				));
			});

			env.pass(Blocks::ByNumber(1));

			env.relay_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Bob.into()
					),
					999907996044
				);
			});
		}

		fn transfer_foreign_sibling_to_altair<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Genesis::default()
					.add(genesis::balances::<T>(air(10)))
					.storage(),
				Default::default(),
			);

			setup_xcm(&mut env);

			let alice_initial_balance = air(10);
			let sibling_asset_id = CurrencyId::ForeignAsset(1);
			let asset_location = MultiLocation::new(
				1,
				X2(Parachain(T::FudgeHandle::SIBLING_ID), general_key(&[0, 1])),
			);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Sibling Native Token".into(),
				symbol: "SBLNG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(asset_location)),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(XcmMetadata {
						// We specify a custom fee_per_second and verify below that this value is
						// used when XCM transfer fees are charged for this token.
						fee_per_second: Some(8420000000000000000),
					}),
					..CustomMetadata::default()
				},
			};
			let transfer_amount = foreign(1, meta.decimals);

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(sibling_asset_id, &Keyring::Bob.into()),
					0
				);
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(CurrencyId::Native),
				));
			});

			env.parachain_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(sibling_asset_id)
				));
			});

			env.sibling_state_mut(|| {
				assert_ok!(pallet_balances::Pallet::<T>::force_set_balance(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Keyring::Alice.to_account_id().into(),
					transfer_amount * 2,
				));

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					CurrencyId::Native,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				let bob_balance =
					orml_tokens::Pallet::<T>::free_balance(sibling_asset_id, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has initial balance + amount transferred - fee
				assert_eq!(
					bob_balance,
					transfer_amount
						- calc_fee(
							xcm_metadata(meta.additional.transferability)
								.unwrap()
								.fee_per_second
								.unwrap()
						)
				);
				// Sanity check to ensure the calculated is what is expected
				assert_eq!(bob_balance, 993264000000000000);
			});
		}

		fn transfer_wormhole_usdc_karura_to_altair<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Default::default(),
				Genesis::default()
					.add(genesis::balances::<T>(air(10)))
					.storage(),
			);

			setup_xcm(&mut env);

			let usdc_asset_id = CurrencyId::ForeignAsset(39);
			let asset_location = MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::SIBLING_ID),
					general_key("0x02f3a00dd12f644daec907013b16eb6d14bf1c4cb4".as_bytes()),
				),
			);
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "Wormhole USDC".into(),
				symbol: "WUSDC".into(),
				existential_deposit: 1,
				location: Some(VersionedMultiLocation::V3(asset_location)),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};
			let transfer_amount = foreign(12, meta.decimals);
			let alice_initial_balance = transfer_amount * 100;

			env.sibling_state_mut(|| {
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(usdc_asset_id)
				));
				assert_ok!(orml_tokens::Pallet::<T>::deposit(
					usdc_asset_id,
					&Keyring::Alice.into(),
					alice_initial_balance
				));
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Alice.into()),
					alice_initial_balance
				);
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					air(10)
				);
			});

			env.parachain_state_mut(|| {
				// First, register the asset in centrifuge
				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(usdc_asset_id)
				));
			});

			env.sibling_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					usdc_asset_id,
					transfer_amount,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(T::FudgeHandle::PARA_ID),
								Junction::AccountId32 {
									network: None,
									id: Keyring::Bob.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000.into()),
				));

				// Confirm that Alice's balance is initial balance - amount transferred
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Alice.into()),
					alice_initial_balance - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				let bob_balance =
					orml_tokens::Pallet::<T>::free_balance(usdc_asset_id, &Keyring::Bob.into());

				// Sanity check to ensure the calculated is what is expected
				assert_eq!(bob_balance, 11992961);
			});
		}

		crate::test_for_runtimes!([altair], test_air_transfers_to_and_from_sibling);
		crate::test_for_runtimes!([altair], transfer_ausd_to_altair);
		crate::test_for_runtimes!([altair], transfer_ksm_to_and_from_relay_chain);
		crate::test_for_runtimes!([altair], transfer_foreign_sibling_to_altair);
		crate::test_for_runtimes!([altair], transfer_wormhole_usdc_karura_to_altair);
	}

	mod asset_registry {
		use super::*;

		fn register_air_works<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Altair".into(),
					symbol: "AIR".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						0,
						X1(general_key(parachains::kusama::altair::AIR_KEY)),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::Native)
				));
			});
		}

		fn register_foreign_asset_works<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 12,
					name: "Acala Dollar".into(),
					symbol: "AUSD".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(
							Parachain(T::FudgeHandle::SIBLING_ID),
							general_key(parachains::kusama::karura::AUSD_KEY),
						),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(CurrencyId::ForeignAsset(42))
				));
			});
		}

		// Verify that registering tranche tokens is not allowed through extrinsics
		fn register_tranche_asset_blocked<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 12,
					name: "Tranche Token 1".into(),
					symbol: "TRNCH".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						X2(Parachain(2000), general_key(&[42])),
					))),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::Xcm(Default::default()),
						..CustomMetadata::default()
					},
				};

				// It fails with `BadOrigin` even when submitted with `Origin::root` since we
				// only allow for tranche tokens to be registered through the pools pallet.
				let asset_id = CurrencyId::Tranche(42, [42u8; 16]);
				assert_noop!(
					orml_asset_registry::Pallet::<T>::register_asset(
						<T as frame_system::Config>::RuntimeOrigin::root(),
						meta,
						Some(asset_id)
					),
					BadOrigin
				);
			});
		}

		crate::test_for_runtimes!([altair], register_air_works);
		crate::test_for_runtimes!([altair], register_foreign_asset_works);
		crate::test_for_runtimes!([altair], register_tranche_asset_blocked);
	}

	mod currency_id_convert {
		use super::*;

		fn convert_air<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			assert_eq!(parachains::kusama::altair::AIR_KEY.to_vec(), vec![0, 1]);

			env.parachain_state_mut(|| {
				// The way AIR is represented relative within the Altair runtime
				let air_location_inner: MultiLocation =
					MultiLocation::new(0, X1(general_key(parachains::kusama::altair::AIR_KEY)));

				// register air
				register_air::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(air_location_inner),
					Ok(CurrencyId::Native),
				);

				// The canonical way AIR is represented out in the wild
				let air_location_canonical: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::PARA_ID),
						general_key(parachains::kusama::altair::AIR_KEY),
					),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
					Some(air_location_canonical)
				)
			});
		}

		/// Verify that Tranche tokens are not handled by the CurrencyIdConvert
		/// since we don't allow Tranche tokens to be transferable through XCM.
		fn convert_tranche<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
			let tranche_id =
				WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
			let tranche_multilocation = MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(T::FudgeHandle::PARA_ID),
					PalletInstance(PoolPalletIndex::get()),
					GeneralKey {
						length: tranche_id.len() as u8,
						data: vec_to_fixed_array(tranche_id.to_vec()),
					},
				),
			};

			env.parachain_state_mut(|| {
				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(tranche_multilocation),
					Err(tranche_multilocation),
				);
			});

			env.parachain_state_mut(|| {
				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(tranche_currency),
					None
				)
			});
		}

		fn convert_ausd<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				assert_eq!(parachains::kusama::karura::AUSD_KEY, &[0, 129]);

				let ausd_location: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::SIBLING_ID),
						general_key(parachains::kusama::karura::AUSD_KEY),
					),
				);

				register_ausd::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(ausd_location.clone()),
					Ok(AUSD_CURRENCY_ID),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(AUSD_CURRENCY_ID),
					Some(ausd_location)
				)
			});
		}

		fn convert_ksm<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let ksm_location: MultiLocation = MultiLocation::parent().into();

			env.parachain_state_mut(|| {
				register_ksm::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(ksm_location),
					Ok(KSM_ASSET_ID),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(KSM_ASSET_ID),
					Some(ksm_location)
				)
			});
		}

		fn convert_unkown_multilocation<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let unknown_location: MultiLocation = MultiLocation::new(
				1,
				X2(Parachain(T::FudgeHandle::PARA_ID), general_key(&[42])),
			);

			env.parachain_state_mut(|| {
				assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location).is_err());
			});
		}

		fn convert_unsupported_currency<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Tranche(
						0,
						[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
					)),
					None
				)
			});
		}

		crate::test_for_runtimes!([altair], convert_air);
		crate::test_for_runtimes!([altair], convert_tranche);
		crate::test_for_runtimes!([altair], convert_ausd);
		crate::test_for_runtimes!([altair], convert_ksm);
		crate::test_for_runtimes!([altair], convert_unkown_multilocation);
		crate::test_for_runtimes!([altair], convert_unsupported_currency);
	}
}
