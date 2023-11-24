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
		AssetId, Fungibility, Junction, Junction::*, Junctions, Junctions::*, MultiAsset,
		MultiAssets, MultiLocation, WeightLimit,
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

	pub fn parachain_account(id: u32) -> AccountId {
		polkadot_parachain::primitives::Sibling::from(id).into_account_truncating()
	}

	pub fn xcm_metadata(transferability: CrossChainTransferability) -> Option<XcmMetadata> {
		match transferability {
			CrossChainTransferability::Xcm(x) | CrossChainTransferability::All(x) => Some(x),
			_ => None,
		}
	}

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
			// Set the XCM version used when sending XCM messages to parachain.
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
}

type FudgeRelayRuntime<T> = <<T as FudgeSupport>::FudgeHandle as FudgeHandle<T>>::RelayRuntime;

use utils::*;

mod altair {
	use altair_runtime::{CurrencyIdConvert, PoolPalletIndex};

	pub const KSM_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(1000);

	use super::*;

	mod utils {
		use super::*;

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
	}

	use utils::*;

	mod transfers {
		use super::*;

		fn transfer_air_to_sibling<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_balance = air(10);
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

mod centrifuge {
	use centrifuge_runtime::CurrencyIdConvert;
	use sp_core::Get;

	use super::*;

	mod utils {
		use xcm::v3::NetworkId;

		use super::*;

		/// The test asset id attributed to DOT
		pub const DOT_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(91);

		pub const LP_ETH_USDC: CurrencyId = CurrencyId::ForeignAsset(100_001);

		pub const USDC: CurrencyId = CurrencyId::ForeignAsset(6);

		/// An Asset that is NOT XCM transferable
		pub const NO_XCM_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(401);

		/// Register DOT in the asset registry.
		/// It should be executed within an externalities environment.
		pub fn register_dot<T: Runtime>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 10,
				name: "Polkadot".into(),
				symbol: "DOT".into(),
				existential_deposit: 100_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::parent())),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};
			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(DOT_ASSET_ID)
			));
		}

		pub fn register_lp_eth_usdc<T: Runtime>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "LP Ethereum Wrapped USDC".into(),
				symbol: "LpEthUSDC".into(),
				existential_deposit: 1_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					0,
					X3(
						PalletInstance(103),
						GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }),
						AccountKey20 {
							network: None,
							key: hex_literal::hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
						},
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::LiquidityPools,
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(LP_ETH_USDC)
			));
		}

		pub fn register_usdc<T: Runtime>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 6,
				name: "USD Circle".into(),
				symbol: "USDC".into(),
				existential_deposit: 1_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X3(
						Junction::Parachain(1000),
						Junction::PalletInstance(50),
						Junction::GeneralIndex(1337),
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
				Some(USDC)
			));
		}

		/// Register AUSD in the asset registry.
		/// It should be executed within an externalities environment.
		pub fn register_ausd<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Acala Dollar".into(),
				symbol: "AUSD".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::SIBLING_ID),
						general_key(parachains::polkadot::acala::AUSD_KEY),
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

		/// Register CFG in the asset registry.
		/// It should be executed within an externalities environment.
		pub fn register_cfg<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Centrifuge".into(),
				symbol: "CFG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::PARA_ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
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

		/// Register CFG in the asset registry as XCM v2, just like it is in
		/// production. It should be executed within an externalities
		/// environment.
		pub fn register_cfg_v2<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Centrifuge".into(),
				symbol: "CFG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V2(xcm::v2::MultiLocation::new(
					1,
					xcm::v2::Junctions::X2(
						xcm::v2::Junction::Parachain(T::FudgeHandle::PARA_ID),
						xcm::v2::Junction::GeneralKey(
							WeakBoundedVec::<u8, ConstU32<32>>::force_from(
								parachains::polkadot::centrifuge::CFG_KEY.into(),
								None,
							),
						),
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

		/// Register a token whose `CrossChainTransferability` does NOT include
		/// XCM.
		pub fn register_no_xcm_token<T: Runtime + FudgeSupport>() {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "NO XCM".into(),
				symbol: "NXCM".into(),
				existential_deposit: 1_000_000_000_000,
				location: None,
				additional: CustomMetadata {
					transferability: CrossChainTransferability::LiquidityPools,
					..CustomMetadata::default()
				},
			};

			assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				meta,
				Some(NO_XCM_ASSET_ID)
			));
		}

		pub fn cfg_fee() -> Balance {
			fee(currency_decimals::NATIVE)
		}

		pub fn ausd_fee() -> Balance {
			fee(currency_decimals::AUSD)
		}

		pub fn fee(decimals: u32) -> Balance {
			calc_fee(default_per_second(decimals))
		}

		// The fee associated with transferring DOT tokens
		pub fn dot_fee() -> Balance {
			fee(10)
		}

		pub fn calc_fee(fee_per_second: Balance) -> Balance {
			// We divide the fee to align its unit and multiply by 4 as that seems to be the
			// unit of time the tests take.
			// NOTE: it is possible that in different machines this value may differ. We
			// shall see.
			fee_per_second.div_euclid(10_000) * 8
		}

		pub fn cfg(amount: Balance) -> Balance {
			amount * dollar(currency_decimals::NATIVE)
		}

		pub fn dollar(decimals: u32) -> Balance {
			10u128.saturating_pow(decimals)
		}

		pub fn ausd(amount: Balance) -> Balance {
			amount * dollar(currency_decimals::AUSD)
		}

		pub fn dot(amount: Balance) -> Balance {
			amount * dollar(10)
		}

		pub fn foreign(amount: Balance, decimals: u32) -> Balance {
			amount * dollar(decimals)
		}

		pub fn transfer_dot_from_relay_chain<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_dot = dot(10);
			let transfer_amount: Balance = dot(3);

			env.parachain_state_mut(|| {
				register_dot::<T>();
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					0
				);
			});

			env.relay_state_mut(|| {
				assert_ok!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::force_set_balance(
						<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
						Keyring::Alice.to_account_id().into(),
						alice_initial_dot,
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
								id: Keyring::Alice.into(),
							}
							.into()
						),
						Box::new((Here, transfer_amount).into()),
						0
					)
				);

				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Alice.into()
					),
					alice_initial_dot - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.parachain_state(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					transfer_amount - dot_fee()
				);
			});
		}
	}

	use utils::*;

	mod asset_registry {
		use super::*;

		fn register_cfg_works<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
					decimals: 18,
					name: "Centrifuge".into(),
					symbol: "CFG".into(),
					existential_deposit: 1_000_000_000_000,
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						0,
						X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
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
							Parachain(parachains::polkadot::acala::ID),
							general_key(parachains::polkadot::acala::AUSD_KEY),
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

		crate::test_for_runtimes!([centrifuge], register_cfg_works);
		crate::test_for_runtimes!([centrifuge], register_foreign_asset_works);
		crate::test_for_runtimes!([centrifuge], register_tranche_asset_blocked);
	}

	mod currency_id_convert {
		use super::*;

		fn convert_cfg<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			assert_eq!(parachains::polkadot::centrifuge::CFG_KEY, &[0, 1]);

			env.parachain_state_mut(|| {
				// The way CFG is represented relative within the Centrifuge runtime
				let cfg_location_inner: MultiLocation = MultiLocation::new(
					0,
					X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
				);

				register_cfg::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(cfg_location_inner),
					Ok(CurrencyId::Native),
				);

				// The canonical way CFG is represented out in the wild
				let cfg_location_canonical: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
					Some(cfg_location_canonical)
				)
			});
		}

		/// Verify that even with CFG registered in the AssetRegistry with a XCM
		/// v2 MultiLocation, that `CurrencyIdConvert` can look it up given an
		/// identical location in XCM v3.
		fn convert_cfg_xcm_v2<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			assert_eq!(parachains::polkadot::centrifuge::CFG_KEY, &[0, 1]);

			env.parachain_state_mut(|| {
				// Registered as xcm v2
				register_cfg_v2::<T>();

				// The way CFG is represented relative within the Centrifuge runtime in xcm v3
				let cfg_location_inner: MultiLocation = MultiLocation::new(
					0,
					X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
				);

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(cfg_location_inner),
					Ok(CurrencyId::Native),
				);

				// The canonical way CFG is represented out in the wild
				let cfg_location_canonical: MultiLocation = MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
					Some(cfg_location_canonical)
				)
			});
		}

		/// Verify that a registered token that is NOT XCM transferable is
		/// filtered out by CurrencyIdConvert as expected.
		fn convert_no_xcm_token<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			env.parachain_state_mut(|| {
				register_no_xcm_token::<T>();

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(NO_XCM_ASSET_ID),
					None
				)
			});
		}

		fn convert_ausd<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			assert_eq!(parachains::polkadot::acala::AUSD_KEY, &[0, 1]);

			let ausd_location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::SIBLING_ID),
					general_key(parachains::polkadot::acala::AUSD_KEY),
				),
			);

			env.parachain_state_mut(|| {
				register_ausd::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(ausd_location),
					Ok(AUSD_CURRENCY_ID),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(AUSD_CURRENCY_ID),
					Some(ausd_location)
				)
			});
		}

		fn convert_dot<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let dot_location: MultiLocation = MultiLocation::parent();

			env.parachain_state_mut(|| {
				register_dot::<T>();

				assert_eq!(
					<CurrencyIdConvert as C1<_, _>>::convert(dot_location),
					Ok(DOT_ASSET_ID),
				);

				assert_eq!(
					<CurrencyIdConvert as C2<_, _>>::convert(DOT_ASSET_ID),
					Some(dot_location)
				)
			});
		}

		fn convert_unknown_multilocation<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			let unknown_location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::PARA_ID),
					general_key([42].as_ref()),
				),
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

		crate::test_for_runtimes!([centrifuge], convert_cfg);
		crate::test_for_runtimes!([centrifuge], convert_cfg_xcm_v2);
		crate::test_for_runtimes!([centrifuge], convert_no_xcm_token);
		crate::test_for_runtimes!([centrifuge], convert_ausd);
		crate::test_for_runtimes!([centrifuge], convert_dot);
		crate::test_for_runtimes!([centrifuge], convert_unknown_multilocation);
		crate::test_for_runtimes!([centrifuge], convert_unsupported_currency);
	}

	mod restricted_transfers {
		use cfg_types::locations::Location;
		use sp_core::Hasher;
		use sp_runtime::traits::BlakeTwo256;

		use super::*;
		use crate::generic::envs::runtime_env::RuntimeEnv;

		const TRANSFER_AMOUNT: u128 = 10;

		fn xcm_location() -> MultiLocation {
			MultiLocation::new(
				1,
				X1(Junction::AccountId32 {
					id: Keyring::Alice.into(),
					network: None,
				}),
			)
		}

		fn allowed_xcm_location() -> Location {
			Location::XCM(BlakeTwo256::hash(&xcm_location().encode()))
		}

		fn add_allowance<T: Runtime>(account: Keyring, asset: CurrencyId, location: Location) {
			assert_ok!(
				pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
					RawOrigin::Signed(account.into()).into(),
					asset,
					location
				)
			);
		}

		fn restrict_lp_eth_usdc_transfer<T: Runtime>() {
			todo!()
		}

		fn restrict_lp_eth_usdc_xcm_transfer<T: Runtime>() {
			todo!()
		}

		fn restrict_usdc_transfer<T: Runtime>() {
			todo!()
		}

		fn restrict_usdc_xcm_transfer<T: Runtime>() {
			todo!()
		}
		fn restrict_dot_transfer<T: Runtime>() {
			let mut env = RuntimeEnv::<T>::from_storage(
				Genesis::default()
					.add(orml_tokens::GenesisConfig::<T> {
						balances: vec![(
							Keyring::Alice.to_account_id(),
							DOT_ASSET_ID,
							T::ExistentialDeposit::get() + dot(TRANSFER_AMOUNT),
						)],
					})
					.storage(),
				Genesis::<T>::default().storage(),
			);

			register_dot::<T>();

			env.parachain_state_mut(|| {
				let pre_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Alice.to_account_id(),
				);
				let pre_transfer_bob = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Bob.to_account_id(),
				);
				let pre_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Charlie.to_account_id(),
				);

				add_allowance::<T>(
					Keyring::Alice,
					DOT_ASSET_ID,
					Location::Local(Keyring::Bob.to_account_id()),
				);

				assert_noop!(
					pallet_restricted_tokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						Keyring::Charlie.into(),
						DOT_ASSET_ID,
						dot(TRANSFER_AMOUNT)
					),
					pallet_transfer_allowlist::Error::<T>::NoAllowanceForDestination
				);

				let after_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Alice.to_account_id(),
				);
				let after_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Charlie.to_account_id(),
				);

				assert_eq!(after_transfer_alice, pre_transfer_alice);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);

				assert_ok!(pallet_restricted_tokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					Keyring::Bob.into(),
					DOT_ASSET_ID,
					dot(TRANSFER_AMOUNT)
				),);

				let after_transfer_alice = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Alice.to_account_id(),
				);
				let after_transfer_bob = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Bob.to_account_id(),
				);
				let after_transfer_charlie = orml_tokens::Pallet::<T>::free_balance(
					DOT_ASSET_ID,
					&Keyring::Charlie.to_account_id(),
				);

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - dot(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + dot(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		fn restrict_dot_xcm_transfer<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			utils::transfer_dot_from_relay_chain(&mut env);

			env.parachain_state_mut(|| {
				let alice_initial_dot =
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into());

				assert_eq!(alice_initial_dot, dot(3) - dot_fee());

				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(1, Junctions::Here)),
					XCM_VERSION,
				));

				assert_ok!(
					pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						DOT_ASSET_ID,
						allowed_xcm_location()
					)
				);

				assert_noop!(
					pallet_restricted_xtokens::Pallet::<T>::transfer(
						RawOrigin::Signed(Keyring::Alice.into()).into(),
						DOT_ASSET_ID,
						dot(1),
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
						WeightLimit::Unlimited,
					),
					pallet_transfer_allowlist::Error::<T>::NoAllowanceForDestination
				);

				assert_ok!(pallet_restricted_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					DOT_ASSET_ID,
					dot(1),
					Box::new(
						MultiLocation::new(
							1,
							X1(Junction::AccountId32 {
								id: Keyring::Alice.into(),
								network: None,
							})
						)
						.into()
					),
					WeightLimit::Unlimited,
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					alice_initial_dot - dot(1),
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.relay_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Alice.into()
					),
					79628418552
				);
			});
		}

		crate::test_for_runtimes!([centrifuge], restrict_dot_transfer);
	}

	mod transfers {
		use super::*;

		fn transfer_cfg_to_sibling<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
			let alice_initial_balance = cfg(10);
			let transfer_amount = cfg(5);
			let cfg_in_sibling = CurrencyId::ForeignAsset(12);

			// CFG Metadata
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Centrifuge".into(),
				symbol: "CFG".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V3(MultiLocation::new(
					1,
					X2(
						Parachain(T::FudgeHandle::PARA_ID),
						general_key(parachains::polkadot::centrifuge::CFG_KEY),
					),
				))),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::Xcm(Default::default()),
					..CustomMetadata::default()
				},
			};

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

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta.clone(),
					Some(CurrencyId::Native),
				));
			});

			env.sibling_state_mut(|| {
				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					0
				);

				assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					meta,
					Some(cfg_in_sibling)
				));
			});

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
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into());

				// Verify that Keyring::Bob now has (amount transferred - fee)
				assert_eq!(current_balance, transfer_amount - fee(18));

				// Sanity check for the actual amount Keyring::Bob ends up with
				assert_eq!(current_balance, 4992960800000000000);
			});
		}

		fn test_cfg_transfers_to_and_from_sibling<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.storage(),
				Default::default(),
			);

			setup_xcm(&mut env);

			// In order to be able to transfer CFG from Sibling to Centrifuge, we need to
			// first send CFG from Centrifuge to Sibling, or else it fails since it'd be
			// like Sibling had minted CFG on their side.
			transfer_cfg_to_sibling(&mut env);

			let alice_initial_balance = cfg(5);
			let bob_initial_balance = cfg(5) - cfg_fee();
			let transfer_amount = cfg(1);
			// Note: This asset was registered in `transfer_cfg_to_sibling`
			let cfg_in_sibling = CurrencyId::ForeignAsset(12);

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
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance
				);
			});

			env.sibling_state_mut(|| {
				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Bob.into()).into(),
					cfg_in_sibling,
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
					orml_tokens::Pallet::<T>::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
					bob_initial_balance - transfer_amount
				);
			});

			env.pass(Blocks::ByNumber(2));

			env.parachain_state_mut(|| {
				// Verify that Keyring::Alice now has initial balance + amount transferred - fee
				assert_eq!(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
					alice_initial_balance + transfer_amount - cfg_fee(),
				);
			});
		}

		fn transfer_ausd_to_centrifuge<T: Runtime + FudgeSupport>() {
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

				// Verify that the amount transferred is now part of the centrifuge parachain
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

		fn transfer_dot_to_and_from_relay_chain<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(Default::default(), Default::default());

			utils::transfer_dot_from_relay_chain(&mut env);

			env.parachain_state_mut(|| {
				let alice_initial_dot =
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into());

				assert_eq!(alice_initial_dot, dot(3) - dot_fee());

				assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
					<T as frame_system::Config>::RuntimeOrigin::root(),
					Box::new(MultiLocation::new(1, Junctions::Here)),
					XCM_VERSION,
				));

				assert_ok!(orml_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					DOT_ASSET_ID,
					dot(1),
					Box::new(
						MultiLocation::new(
							1,
							X1(Junction::AccountId32 {
								id: Keyring::Alice.into(),
								network: None,
							})
						)
						.into()
					),
					WeightLimit::Unlimited,
				));

				assert_eq!(
					orml_tokens::Pallet::<T>::free_balance(DOT_ASSET_ID, &Keyring::Alice.into()),
					alice_initial_dot - dot(1),
				);
			});

			env.pass(Blocks::ByNumber(1));

			env.relay_state_mut(|| {
				assert_eq!(
					pallet_balances::Pallet::<FudgeRelayRuntime<T>>::free_balance(
						&Keyring::Alice.into()
					),
					79628418552
				);
			});
		}

		fn transfer_foreign_sibling_to_centrifuge<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
					.storage(),
				Default::default(),
			);

			setup_xcm(&mut env);

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
				// First, register the asset in centrifuge
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

		fn transfer_wormhole_usdc_acala_to_centrifuge<T: Runtime + FudgeSupport>() {
			let mut env = FudgeEnv::<T>::from_storage(
				Default::default(),
				Genesis::default()
					.add(genesis::balances::<T>(cfg(10)))
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
					cfg(10)
				);
			});

			env.parachain_state_mut(|| {
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

		crate::test_for_runtimes!([centrifuge], test_cfg_transfers_to_and_from_sibling);
		crate::test_for_runtimes!([centrifuge], transfer_ausd_to_centrifuge);
		crate::test_for_runtimes!([centrifuge], transfer_dot_to_and_from_relay_chain);
		crate::test_for_runtimes!([centrifuge], transfer_foreign_sibling_to_centrifuge);
		crate::test_for_runtimes!([centrifuge], transfer_wormhole_usdc_acala_to_centrifuge);
	}
}

mod all {
	use super::*;

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

		crate::test_for_runtimes!(all, xtokens_transfer);
		crate::test_for_runtimes!(all, xtokens_transfer_multiasset);
		crate::test_for_runtimes!(all, xtokens_transfer_multiassets);
	}
}
