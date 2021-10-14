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

//! Unit test cases for Loan pallet

use super::*;
use crate as pallet_loan;
use crate::mock::TestExternalitiesBuilder;
use crate::mock::{Event, Loan, MockRuntime, Origin};
use frame_support::{assert_err, assert_ok};
use pallet_loan::Event as LoanEvent;
use pallet_registry::traits::VerifierRegistry;
use runtime_common::{Amount, AssetInfo, PoolId, Rate, TokenId};
use sp_runtime::traits::One;

fn create_nft_registry<T>(owner: AccountIdOf<T>) -> RegistryIdOf<T>
where
	T: frame_system::Config + pallet_nft::Config + pallet_loan::Config,
{
	let registry_info = RegistryInfo {
		owner_can_burn: false,
		fields: vec![],
	};

	// Create registry, get registry id. Shouldn't fail.
	<T as pallet_loan::Config>::VaRegistry::create_new_registry(owner, registry_info.clone())
}

fn mint_nft<T>(owner: AccountIdOf<T>, registry_id: RegistryIdOf<T>) -> TokenIdOf<T>
where
	T: frame_system::Config
		+ pallet_nft::Config<TokenId = TokenId, AssetInfo = AssetInfo>
		+ pallet_loan::Config,
{
	let token_id = TokenId(U256::one());
	let asset_id = AssetId(registry_id, token_id);
	let asset_info = AssetInfo::default();
	let caller = owner.clone();
	<T as pallet_loan::Config>::NftRegistry::mint(caller, owner, asset_id, asset_info)
		.expect("mint should not fail");
	token_id.into()
}

fn create_pool<T>(owner: AccountIdOf<T>) -> PoolId
where
	T: pallet_pool::Config<PoolId = PoolId> + frame_system::Config,
{
	pallet_pool::Pallet::<T>::create_new_pool(owner, "some pool".into())
}

// Return last triggered event
fn last_event() -> Event {
	frame_system::Pallet::<MockRuntime>::events()
		.pop()
		.map(|item| item.event)
		.expect("Event expected")
}

fn expect_event<E: Into<Event>>(event: E) {
	assert_eq!(last_event(), event.into());
}

fn expect_asset_owner<T: frame_system::Config + pallet_loan::Config>(
	asset_id: AssetIdOf<T>,
	owner: AccountIdOf<T>,
) {
	assert_eq!(
		<T as pallet_loan::Config>::NftRegistry::owner_of(asset_id).unwrap(),
		owner
	);
}

fn fetch_loan_event(event: Event) -> Option<LoanEvent<MockRuntime>> {
	match event {
		Event::Loan(loan_event) => Some(loan_event),
		_ => None,
	}
}

#[test]
fn issue_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let owner: u64 = 1;
			let pool_id = create_pool::<MockRuntime>(owner);
			let asset_registry = create_nft_registry::<MockRuntime>(owner);
			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
			let res = Loan::issue_loan(
				Origin::signed(owner),
				pool_id,
				AssetId(asset_registry, token_id),
			);
			assert_ok!(res);

			// post issue checks
			// token nonce should 2
			assert_eq!(NextLoanNftTokenID::<MockRuntime>::get(), 2u128.into());

			// loanId should be 1
			let loan_id: LoanIdOf<MockRuntime> = TokenId(U256::one());
			// event should be emitted
			expect_event(LoanEvent::LoanIssued(pool_id, loan_id.into()));
			let loan_data =
				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");

			// asset is same as we sent before
			assert_eq!(loan_data.asset_id, AssetId(asset_registry, token_id));
			assert_eq!(loan_data.status, LoanStatus::Issued);

			// asset owner is loan pallet
			expect_asset_owner::<MockRuntime>(
				AssetId(asset_registry, token_id),
				Loan::account_id(),
			);

			// wrong owner
			let owner2 = 2;
			let res = Loan::issue_loan(
				Origin::signed(owner2),
				pool_id,
				AssetId(asset_registry, token_id),
			);
			assert_err!(res, Error::<MockRuntime>::ErrNotNFTOwner);

			// missing owner
			let token_id = TokenId(100u128.into());
			let res = Loan::issue_loan(
				Origin::signed(owner2),
				pool_id,
				AssetId(asset_registry, token_id),
			);
			assert_err!(res, Error::<MockRuntime>::ErrNFTOwnerNotFound);

			// trying to issue a loan with loan nft
			let loan_nft_registry = PoolToLoanNftRegistry::<MockRuntime>::get(pool_id)
				.expect("Registry should be created");
			let res = Loan::issue_loan(
				Origin::signed(owner),
				pool_id,
				AssetId(loan_nft_registry, loan_id),
			);
			assert_err!(res, Error::<MockRuntime>::ErrNotAValidAsset)
		});
}

#[test]
fn activate_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let owner: u64 = 100;
			let pool_id = create_pool::<MockRuntime>(owner);
			let asset_registry = create_nft_registry::<MockRuntime>(owner);
			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
			let res = Loan::issue_loan(
				Origin::signed(owner),
				pool_id,
				AssetId(asset_registry, token_id),
			);
			assert_ok!(res);

			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (pool_id, loan_id) = match loan_event {
				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
				_ => None,
			}
			.expect("must be a Loan issue event");

			let oracle: u64 = 1;
			let res = Loan::activate_loan(
				Origin::signed(oracle),
				pool_id,
				loan_id,
				Rate::one(),
				Amount::from_inner(100u128),
				None,
			);
			assert_ok!(res);
			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (pool_id, loan_id) = match loan_event {
				LoanEvent::LoanActivated(pool_id, loan_id) => Some((pool_id, loan_id)),
				_ => None,
			}
			.expect("must be a Loan issue activated event");
			// check loan status as Activated
			let loan_data =
				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
			assert_eq!(loan_data.status, LoanStatus::Active);
			assert_eq!(loan_data.maturity_date, None);
			assert_eq!(loan_data.rate_per_sec, Rate::one());
			assert_eq!(loan_data.ceiling, Amount::from_inner(100u128));
		})
}

#[test]
fn close_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let owner: u64 = 1;
			let pool_id = create_pool::<MockRuntime>(owner);
			let asset_registry = create_nft_registry::<MockRuntime>(owner);
			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
			let res = Loan::issue_loan(
				Origin::signed(owner),
				pool_id,
				AssetId(asset_registry, token_id),
			);
			assert_ok!(res);

			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (pool_id, loan_id) = match loan_event {
				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
				_ => None,
			}
			.expect("must be a Loan issue event");

			// activate loan
			let oracle: u64 = 1;
			let res = Loan::activate_loan(
				Origin::signed(oracle),
				pool_id,
				loan_id,
				Rate::one(),
				Amount::from_inner(100u128),
				None,
			);
			assert_ok!(res);

			// close the loan
			let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
			assert_ok!(res);

			let (pool_id, loan_id, asset) = match fetch_loan_event(last_event())
				.expect("should be a loan event")
			{
				LoanEvent::LoanClosed(pool_id, loan_id, asset) => Some((pool_id, loan_id, asset)),
				_ => None,
			}
			.expect("must be a Loan close event");
			assert_eq!(asset, AssetId(asset_registry, token_id));

			// check asset owner
			expect_asset_owner::<MockRuntime>(AssetId(asset_registry, token_id), owner);

			// check nft loan owner loan pallet
			let loan_nft_registry = PoolToLoanNftRegistry::<MockRuntime>::get(pool_id)
				.expect("must have a loan nft registry created");
			expect_asset_owner::<MockRuntime>(
				AssetId(loan_nft_registry, loan_id),
				Loan::account_id(),
			);

			// check loan status as Closed
			let loan_data =
				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
			assert_eq!(loan_data.status, LoanStatus::Closed);
		})
}
