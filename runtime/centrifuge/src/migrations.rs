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

use frame_support::migrations::VersionedMigration;
use sp_core::{parameter_types, H160};

use crate::Runtime;

parameter_types! {
	pub PalletLiquidityPoolsAxelarGateway: &'static str = "LiquidityPoolsAxelarGateway";
	pub AxelarGatewayContract: H160 = hex_literal::hex!("4F4495243837681061C4743b74B3eEdf548D56A5").into();
	pub ForwarderContract: H160 = hex_literal::hex!("c1757c6A0563E37048869A342dF0651b9F267e41").into();
}

/// The migration set for Centrifuge @ Polkadot.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeCentrifuge1505 = (
	// Remove deprecated LiquidityPoolsGateway::{v0, v1, v2}::RelayerList storage
	runtime_common::migrations::liquidity_pools_v2::kill_relayer_list::Migration<Runtime>,
	// Clear OutboundMessageNonceStore and migrate outbound storage to LP queue
	runtime_common::migrations::liquidity_pools_v2::v0_init_message_queue::Migration<Runtime>,
	// Remove deprecated DomainRouters entries and migrate relevant ones to Axelar Router Config
	VersionedMigration<
		0,
		3,
		runtime_common::migrations::liquidity_pools_v2::init_axelar_router::Migration<
			Runtime,
			AxelarGatewayContract,
			ForwarderContract,
		>,
		pallet_liquidity_pools_gateway::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Remove deprecated LiquidityPoolsGateway::{v0, v1, v2}::Allowlist storage
	runtime_common::migrations::liquidity_pools_v2::kill_allowlist::Migration<Runtime, 20>,
	// Remove undecodable ForeignInvestmentInfo v0 entries
	runtime_common::migrations::foreign_investments_v2::Migration<Runtime>,
	// Bump to v1
	runtime_common::migrations::increase_storage_version::Migration<
		pallet_foreign_investments::Pallet<Runtime>,
		1,
		2,
	>,
	// Migrate TrancheInvestor permission role and storage version from v0 to v1
	frame_support::migrations::VersionedMigration<
		0,
		1,
		runtime_common::migrations::permissions_v1::Migration<
			Runtime,
			crate::MinDelay,
			crate::MaxTranches,
		>,
		pallet_permissions::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Remove deprecated LiquidityPoolsAxelarGateway
	runtime_common::migrations::nuke::KillPallet<
		PalletLiquidityPoolsAxelarGateway,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Rename Local USDC to US Dollar, register DAI and USDS
	VersionedMigration<
		0,
		1,
		runtime_common::migrations::asset_registry_local_usdc_dai_usds::CombinedMigration<Runtime>,
		pallet_token_mux::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Re-reset council according to CP136
	reset_council::Migration<Runtime>,
	// Clear voting locks from elections
	remove_phragmen_votes::Migration<Runtime>,
);

mod remove_phragmen_votes {
	#[cfg(feature = "try-runtime")]
	use frame_support::storage::transactional;
	use frame_support::{
		traits::{Get, LockableCurrency, OnRuntimeUpgrade},
		weights::Weight,
	};
	#[cfg(feature = "try-runtime")]
	use sp_runtime::traits::Zero;
	use sp_runtime::Saturating;
	#[cfg(feature = "try-runtime")]
	use sp_std::{vec, vec::Vec};

	const LOG_PREFIX: &str = "RemovePhragmenVotes";

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: frame_system::Config + pallet_elections_phragmen::Config + pallet_balances::Config,
	{
		fn on_runtime_upgrade() -> Weight {
			Self::migrate()
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			let count = pallet_elections_phragmen::Voting::<T>::iter_keys().count();

			// NOTE: Block execution for idempotency checks which operate on migrated state
			if !count.is_zero() {
				let _ = transactional::with_storage_layer(|| -> sp_runtime::DispatchResult {
					log::info!("{LOG_PREFIX}: Pre init migration");
					Self::migrate();
					Err(sp_runtime::DispatchError::Other("Reverting on purpose"))
				});
			} else {
				log::info!("{LOG_PREFIX}: Voting count is zero, migration can be removed");
			}

			log::info!("{LOG_PREFIX}: Pre done");
			Ok(vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			let count = pallet_elections_phragmen::Voting::<T>::iter_keys().count();
			assert_eq!(count, 0, "Voting should be cleared");

			let lock_count = pallet_balances::Locks::<T>::iter_values()
				.filter(|locks| {
					locks
						.into_iter()
						.filter(|lock| {
							lock.id == <T as pallet_elections_phragmen::Config>::PalletId::get()
						})
						.count() > 0
				})
				.count();
			assert_eq!(
				lock_count, 0,
				"Elections locks should be cleared from balances",
			);

			log::info!("{LOG_PREFIX}: Post done");

			Ok(())
		}
	}

	impl<T> Migration<T>
	where
		T: frame_system::Config + pallet_elections_phragmen::Config + pallet_balances::Config,
	{
		fn migrate() -> Weight {
			let mut voters: u64 = 0;

			for (voter, _) in pallet_elections_phragmen::Voting::<T>::drain() {
				log::debug!("{LOG_PREFIX}: Removing phragmens lock of voter {voter:?}");
				pallet_balances::Pallet::<T>::remove_lock(
					<T as pallet_elections_phragmen::Config>::PalletId::get(),
					&voter,
				);
				voters.saturating_accrue(1);
			}

			log::info!("{LOG_PREFIX}: Removed {voters} many locks");

			T::DbWeight::get().reads_writes(voters, voters.saturating_mul(2))
		}
	}
}

mod reset_council {
	use cfg_primitives::AccountId;
	#[cfg(feature = "try-runtime")]
	use frame_support::storage::transactional;
	use frame_support::{
		traits::{Get, OnRuntimeUpgrade, OriginTrait},
		weights::Weight,
	};
	use pallet_collective::pallet::Pallet as PalletCouncil;
	use runtime_common::instances::CouncilCollective;
	use sp_core::crypto::AccountId32;
	use sp_std::{vec, vec::Vec};

	use super::*;

	const LOG_PREFIX: &str = "ResetCouncil";

	parameter_types! {
		// William = 4dhWqvsRVE8urtAcn3RkbT1oJnqFktGF1abfvuhyC8Z13Lnd
		pub PrimeVoter: AccountId = hex_literal::hex!("684f4dc6a026ea82a6cb36de4330a1a44428bbe243fb7f26ccf6227b0d0ef054").into();
		pub CouncilMembers: Vec<AccountId> = vec![
			// Ash = 4fU7xg1tWQhcyZKD2YX4B9KrHpsC8pUjkWLApsTcvV7VznmD
			hex_literal::hex!("b6908c066b96674d41ab8a2c3995a19a1351f252bfd98ce62f1d981d9feac13f").into(),
			// Ivan = 4bj22Bq3C9AvpFne6HXD5KDfqWbYCYFNoxWEiNWjCXtzLfK8
			hex_literal::hex!("10fc4ce597a78de5984781b99ce5e6f89807bc961fa5f510876bc214d6e19755").into(),
			// Lucas = 4fYrN4QvRaBf6HpWShZnFR7hNsGSEBHDw1115jYQqQE5SkTL
			hex_literal::hex!("ba2c4540acac96a93e611ec4258ce05338434f12107d35f29783bbd2477dd20e").into(),
			// Luis G = 4ecNXNZcmdiDahSGNZiZQ8zKj2rCiUxuuQtrME16G2NfAiYn
			hex_literal::hex!("909f4d84481e5466f7af1143a7193f40bd8c0606d75eb089666c29ca0347606a").into(),
			// Luis E = 4ck67NuZLjvbMRijqsmHdRMbGbyq2CoD99urmawqvx73WUn4
			hex_literal::hex!("3e098bb449c1ab045c84e560c301a04ecd10660b7411b649047c8ca247115265").into(),
			// Miguel = 4eCFcfJMuzEyeJzG54zxYv5b3X34582vLbv7cfGHUwNvsW6h
			hex_literal::hex!("7e3a27ebc30843a9b856bcd77423bd10db0dd98caa295e4dbe87783dcfd3e939").into(),
			// Orhan = 4h68S4rjBe57VzDUc1dzMXd3FMXvTfLteoRgkBWMc2XGh4Xa
			hex_literal::hex!("fe433475123c3ccf28162438376e52c16f7a2f14d9d86257691734e5708fa001").into(),
			// Yaroslav = 4fWLgjzJWSaWuRotm8J8Ma6PL7276UcPPLFnPn9nWdYN2QCL
			hex_literal::hex!("b841e0018ac8e4b232a8c72b94b460f2b02e6a89fd1745f44dbfde5120ee300c").into(),
			PrimeVoter::get(),
		];
		pub const OldCount: u32 = 8;
	}

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: frame_system::Config<AccountId = AccountId32>
			+ pallet_collective::Config<CouncilCollective>,
	{
		fn on_runtime_upgrade() -> Weight {
			Self::migrate()
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			// NOTE: Block execution for idempotency checks which operate on migrated state
			if pallet_collective::Pallet::<T, CouncilCollective>::prime() != Some(PrimeVoter::get())
			{
				let member_count =
					pallet_collective::Pallet::<T, CouncilCollective>::members().len();
				assert_eq!(member_count, OldCount::get() as usize, "OldCount mismatch");

				let _ = transactional::with_storage_layer(|| -> sp_runtime::DispatchResult {
					Self::migrate();
					Err(sp_runtime::DispatchError::Other("Reverting on purpose"))
				});
			} else {
				log::info!("{LOG_PREFIX}: Council already reset, migration can be removed");
			}

			log::info!("{LOG_PREFIX}: Pre done");

			Ok(vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			let members = pallet_collective::Pallet::<T, CouncilCollective>::members();

			for new_member in CouncilMembers::get().iter() {
				log::debug!("{LOG_PREFIX}: Checking for {new_member:?}");
				assert!(
					members
						.iter()
						.any(|check_member| check_member == new_member),
					"New member missing in updated Council"
				);
			}

			assert_eq!(
				pallet_collective::Pallet::<T, CouncilCollective>::prime(),
				Some(PrimeVoter::get()),
				"Prime Voter mismatch"
			);

			log::info!("{LOG_PREFIX}: Post done");

			Ok(())
		}
	}

	impl<T> Migration<T>
	where
		T: frame_system::Config<AccountId = AccountId32>
			+ pallet_collective::Config<CouncilCollective>,
	{
		fn migrate() -> Weight {
			PalletCouncil::<T, CouncilCollective>::set_members(
				<T as frame_system::Config>::RuntimeOrigin::root(),
				CouncilMembers::get(),
				Some(PrimeVoter::get()),
				OldCount::get(),
			)
			.map_err(|e| {
				log::error!("{LOG_PREFIX}: Failed to reset council due to error {:?}", e);
			})
			.map(|info| info.actual_weight)
			.ok()
			.flatten()
			.unwrap_or(
				T::DbWeight::get().writes(
					// Remove Voting for each old member
					(OldCount::get() as u64)
						// Set new members, kill prime, set prime
						.saturating_add(3),
				),
			)
		}
	}
}
