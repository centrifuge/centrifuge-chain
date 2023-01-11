// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{AccountId, CollectionId, PoolId, CFG};
use cfg_traits::{Permissions, PoolMutate};
use cfg_types::{
	fixed_point::Rate,
	permissions::{PermissionScope, PoolRole, Role},
	tokens::{CurrencyId, CustomMetadata},
};
use development_runtime::{apis::PoolsApi, RuntimeOrigin};
use frame_benchmarking::account;
use frame_support::{
	assert_ok,
	dispatch::RawOrigin,
	traits::{
		tokens::nonfungibles::{Create, Mutate as NMutate},
		OriginTrait,
	},
};
use orml_traits::asset_registry::Mutate;
use pallet_loans::types::Asset;
use pallet_pool_system::tranches::{TrancheInput, TrancheMetadata, TrancheType};
use sp_core::{bounded::BoundedVec, sr25519, Pair};
use sp_runtime::{
	traits::{IdentifyAccount, One},
	Perquintill,
};
use tokio::runtime::Handle;

use super::ApiEnv;
use crate::pools::utils::loans::LoanId;

#[tokio::test]
async fn test() {
	ApiEnv::new(Handle::current())
		.startup(|| {
			let pool_id = 3;

			let admin = sp_runtime::AccountId32::from(
				<sr25519::Pair as sp_core::Pair>::from_string("//Alice", None)
					.unwrap()
					.public()
					.into_account(),
			);

			let borrower = sp_runtime::AccountId32::from(
				<sr25519::Pair as sp_core::Pair>::from_string("//Bob", None)
					.unwrap()
					.public()
					.into_account(),
			);

			let admin_account = account::<AccountId>("//Alice", 0, 0);
			let borrower_account = account::<AccountId>("//Bob", 0, 0);

			<development_runtime::Balances>::set_balance(
				RuntimeOrigin::root(),
				sp_runtime::MultiAddress::Id(admin_account),
				10000 * 10_000_000_000,
				0,
			)
			.unwrap();

			<development_runtime::Balances>::set_balance(
				RuntimeOrigin::root(),
				sp_runtime::MultiAddress::Id(borrower_account),
				10000 * 10_000_000_000,
				0,
			)
			.unwrap();

			<development_runtime::Permissions as Permissions<AccountId>>::add(
				PermissionScope::Pool(pool_id),
				borrower.clone(),
				Role::PoolRole(PoolRole::Borrower),
			);

			let token_name = BoundedVec::try_from("SuperToken".as_bytes().to_owned())
				.expect("Can't create BoundedVec");
			let token_symbol =
				BoundedVec::try_from("ST".as_bytes().to_owned()).expect("Can't create BoundedVec");

			// Setting up metadata for the pool currency
			<development_runtime::OrmlAssetRegistry as Mutate>::register_asset(
				Some(CurrencyId::AUSD.into()),
				orml_asset_registry::AssetMetadata {
					decimals: 18,
					name: token_name.to_vec(),
					symbol: token_symbol.to_vec(),
					existential_deposit: 0_u128.into(),
					location: None,
					additional: CustomMetadata::default(),
				},
			)
			.expect("Registering asset metadata should not fail");

			// Creating a pool
			<development_runtime::PoolSystem as PoolMutate<AccountId, PoolId>>::create(
				admin.clone(),
				admin.clone(),
				pool_id.clone(),
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name,
							token_symbol,
						},
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: Rate::one(),
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						},
					},
				],
				CurrencyId::AUSD,
				10_000 * 10_000_000_000,
				None,
			)
			.expect("Pool creation should not fail");

			//
			// Initalising a pool and creating a loan
			//
			let loan_nft_class_id: CollectionId = 1_u64.into();
			<development_runtime::Uniques as Create<AccountId>>::create_collection(
				&loan_nft_class_id,
				&admin.clone(),
				&admin.clone(),
			)
			.expect("collection creation should not fail");

			<development_runtime::Loans>::initialise_pool(
				RawOrigin::Signed(admin).into(),
				pool_id,
				loan_nft_class_id,
			)
			.expect("initialisation of pool should not fail");

			let collateral_class: CollectionId = 2_u64.into();
			<development_runtime::Uniques as Create<AccountId>>::create_collection(
				&collateral_class,
				&borrower.clone(),
				&borrower.clone(),
			)
			.expect("class creation should not fail");

			let instance_id = LoanId::from(1_u32);

			<development_runtime::Uniques as NMutate<AccountId>>::mint_into(
				&collateral_class.into(),
				&instance_id.into(),
				&borrower,
			)
			.expect("mint should not fail");

			let collateral = Asset(collateral_class, instance_id);

			<development_runtime::Loans>::create(
				RuntimeOrigin::signed(borrower),
				pool_id,
				collateral,
			)
			.expect("Creation of loan should not fail");
		})
		.with_api(|api, latest| {
			let valuation = api.portfolio_valuation(&latest, 3).unwrap();
			assert_eq!(valuation, Some(0));
		});
}
