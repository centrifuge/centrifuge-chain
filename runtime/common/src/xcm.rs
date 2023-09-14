// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
use cfg_primitives::types::Balance;
use cfg_traits::TryConvert;
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	EVMChainId, ParaId,
};
use frame_support::sp_std::marker::PhantomData;
use sp_core::H160;
use sp_runtime::traits::Convert;
use xcm::v3::{
	Junction::{AccountId32, AccountKey20, GeneralKey, Parachain},
	Junctions::{X1, X2},
	MultiLocation, OriginKind,
};

use crate::xcm_fees::default_per_second;

/// Our FixedConversionRateProvider, used to charge XCM-related fees for
/// tokens registered in the asset registry that were not already handled by
/// native Trader rules.
pub struct FixedConversionRateProvider<OrmlAssetRegistry>(PhantomData<OrmlAssetRegistry>);

impl<
		OrmlAssetRegistry: orml_traits::asset_registry::Inspect<
			AssetId = CurrencyId,
			Balance = Balance,
			CustomMetadata = CustomMetadata,
		>,
	> orml_traits::FixedConversionRateProvider for FixedConversionRateProvider<OrmlAssetRegistry>
{
	fn get_fee_per_second(location: &MultiLocation) -> Option<u128> {
		let metadata = OrmlAssetRegistry::metadata_by_location(location)?;
		match metadata.additional.transferability {
			CrossChainTransferability::Xcm(xcm_metadata)
			| CrossChainTransferability::All(xcm_metadata) => xcm_metadata
				.fee_per_second
				.or_else(|| Some(default_per_second(metadata.decimals))),
			_ => None,
		}
	}
}

/// A utils function to un-bloat and simplify the instantiation of
/// `GeneralKey` values
pub fn general_key(data: &[u8]) -> xcm::latest::Junction {
	GeneralKey {
		length: data.len().min(32) as u8,
		data: cfg_utils::vec_to_fixed_array(data.to_vec()),
	}
}

/// How we convert an `[AccountId]` into an XCM MultiLocation
pub struct AccountIdToMultiLocation<AccountId>(PhantomData<AccountId>);
impl<AccountId> Convert<AccountId, MultiLocation> for AccountIdToMultiLocation<AccountId>
where
	AccountId: Into<[u8; 32]>,
{
	fn convert(account: AccountId) -> MultiLocation {
		X1(AccountId32 {
			network: None,
			id: account.into(),
		})
		.into()
	}
}

pub struct LpInstanceRelayer<ParaAsEvmChain, Runtime>(PhantomData<(ParaAsEvmChain, Runtime)>);
impl<ParaAsEvmChain, Runtime>
	xcm_executor::traits::ConvertOrigin<<Runtime as frame_system::Config>::RuntimeOrigin>
	for LpInstanceRelayer<ParaAsEvmChain, Runtime>
where
	ParaAsEvmChain: TryConvert<ParaId, EVMChainId>,
	Runtime: pallet_liquidity_pools_gateway::Config,
	<Runtime as frame_system::Config>::RuntimeOrigin:
		From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
	fn convert_origin(
		origin: impl Into<MultiLocation>,
		kind: OriginKind,
	) -> Result<<Runtime as frame_system::Config>::RuntimeOrigin, MultiLocation> {
		let location: MultiLocation = origin.into();
		match kind {
			OriginKind::SovereignAccount => match location {
				MultiLocation {
					parents: 1,
					interior: X2(Parachain(para), AccountKey20 { key, .. }),
				} => {
					let evm_id = ParaAsEvmChain::try_convert(para).map_err(|_| location)?;
					let domain_address = DomainAddress::EVM(evm_id, key);

					if pallet_liquidity_pools_gateway::Pallet::<Runtime>::relayer(
						Domain::EVM(evm_id),
						&domain_address,
					)
					.is_some()
					{
						Ok(pallet_liquidity_pools_gateway::GatewayOrigin::AxelarRelay(
							domain_address,
						)
						.into())
					} else {
						Err(location)
					}
				}
				// IMPORTANT - This only applies in our integration test environment since the
				// `Moonbeam` parachain that we setup there is using the same AccountId as we do on
				// Centrifuge, which is 32 bytes instead of 20.
				//
				// !!! REMOVE BEFORE MERGING !!!
				MultiLocation {
					parents: 1,
					interior: X2(Parachain(para), AccountId32 { network: _, id }),
				} => {
					let evm_id = ParaAsEvmChain::try_convert(para).map_err(|_| location)?;

					let domain_address = DomainAddress::EVM(
						evm_id,
						H160::from_slice(&id.as_ref()[0..20]).to_fixed_bytes(),
					);

					if pallet_liquidity_pools_gateway::Pallet::<Runtime>::relayer(
						Domain::EVM(evm_id),
						&domain_address,
					)
					.is_some()
					{
						Ok(pallet_liquidity_pools_gateway::GatewayOrigin::AxelarRelay(
							domain_address,
						)
						.into())
					} else {
						Err(location)
					}
				}
				_ => Err(location),
			},
			_ => Err(location),
		}
	}
}

#[cfg(test)]
mod test {
	use cfg_mocks::{
		pallet_mock_liquidity_pools, pallet_mock_routers, pallet_mock_try_convert, MessageMock,
		RouterMock,
	};
	use frame_support::{assert_ok, traits::EnsureOrigin};
	use frame_system::EnsureRoot;
	use pallet_liquidity_pools_gateway::{EnsureLocal, GatewayOrigin};
	use sp_core::{ConstU16, ConstU32, ConstU64, H256};
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
		DispatchError,
	};
	use xcm_executor::traits::ConvertOrigin;

	use super::*;

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type AccountId = u64;

	pub fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	// For testing the pallet, we construct a mock runtime.
	frame_support::construct_runtime!(
		pub enum Runtime where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system,
			Gateway: pallet_liquidity_pools_gateway,
			MockLP: pallet_mock_liquidity_pools,
			MockParaAsEvmChain: pallet_mock_try_convert::<Instance1>,
			MockOriginRecovery: pallet_mock_try_convert::<Instance2>,
		}
	);

	impl frame_system::Config for Runtime {
		type AccountData = ();
		type AccountId = AccountId;
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockHashCount = ConstU64<250>;
		type BlockLength = ();
		type BlockNumber = u64;
		type BlockWeights = ();
		type DbWeight = ();
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Header = Header;
		type Index = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type MaxConsumers = ConstU32<16>;
		type OnKilledAccount = ();
		type OnNewAccount = ();
		type OnSetCode = ();
		type PalletInfo = PalletInfo;
		type RuntimeCall = RuntimeCall;
		type RuntimeEvent = RuntimeEvent;
		type RuntimeOrigin = RuntimeOrigin;
		type SS58Prefix = ConstU16<42>;
		type SystemWeightInfo = ();
		type Version = ();
	}

	impl pallet_mock_try_convert::Config<pallet_mock_try_convert::Instance1> for Runtime {
		type Error = ();
		type From = ParaId;
		type To = EVMChainId;
	}

	impl pallet_mock_try_convert::Config<pallet_mock_try_convert::Instance2> for Runtime {
		type Error = DispatchError;
		type From = (Vec<u8>, Vec<u8>);
		type To = DomainAddress;
	}

	impl pallet_mock_liquidity_pools::Config for Runtime {
		type DomainAddress = DomainAddress;
		type Message = MessageMock;
	}

	impl pallet_mock_routers::Config for Runtime {}

	impl pallet_liquidity_pools_gateway::Config for Runtime {
		type AdminOrigin = EnsureRoot<AccountId>;
		type InboundQueue = MockLP;
		type LocalEVMOrigin = pallet_liquidity_pools_gateway::EnsureLocal;
		type MaxIncomingMessageSize = ConstU32<1024>;
		type Message = MessageMock;
		type OriginRecovery = MockOriginRecovery;
		type Router = RouterMock<Runtime>;
		type RuntimeEvent = RuntimeEvent;
		type RuntimeOrigin = RuntimeOrigin;
		type Sender = ConstU64<11>;
		type WeightInfo = ();
	}

	const RELAYER_PARA_ID: u32 = 1000;
	const RELAYER_EVM_ID: u64 = 1001;
	const RELAYER_ADDRESS: [u8; 20] = [1u8; 20];

	#[test]
	fn lp_instance_relayer_converts_correctly() {
		new_test_ext().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				),
			);

			let origin = LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
				location,
				OriginKind::SovereignAccount,
			)
			.expect("Origin conversion failed unexpectedly.");

			assert_eq!(
				EnsureLocal::ensure_origin(origin).expect("Generate origin must be GatewayOrigin"),
				GatewayOrigin::AxelarRelay(expected_address)
			)
		})
	}

	#[test]
	fn lp_instance_relayer_fails_with_wrong_location() {
		new_test_ext().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location: MultiLocation = MultiLocation::new(1, X1(Parachain(RELAYER_PARA_ID)));

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location,
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_relayer_not_set() {
		new_test_ext().execute_with(|| {
			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				),
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location,
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_para_to_evm_fails() {
		new_test_ext().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Err(())
			});

			let location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				),
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location,
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_wrong_para() {
		new_test_ext().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, 1);
				Err(())
			});

			let location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(1),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				),
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location,
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_wrong_address() {
		new_test_ext().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location: MultiLocation = MultiLocation::new(
				1,
				X2(
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: [0u8; 20],
					},
				),
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location,
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}
}
