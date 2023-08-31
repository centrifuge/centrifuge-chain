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
				_ => Err(location),
			},
			_ => Err(location),
		}
	}
}

#[cfg(test)]
mod test {
	#[test]
	fn lp_gatway_converts_correctly() {}
}
