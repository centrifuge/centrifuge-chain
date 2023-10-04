// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use frame_support::{
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
};
use sp_arithmetic::traits::{EnsureAdd, EnsureMul};
use sp_core::H160;

use crate::evm::precompile::PRECOMPILE_CODE_STORAGE;

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet_evm::Config> OnRuntimeUpgrade for Migration<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!("precompile::AccountCodes: Inserting precompile account codes: on_runtime_upgrade: started");

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::ECRECOVER_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for ECRECOVER.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::ECRECOVER_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: ECRECOVER storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::SHA256_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for SHA256.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::SHA256_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: SHA256 storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::RIPEMD160_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for RIPEMD160.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::RIPEMD160_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: RIPEMD160 storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::IDENTITY_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for IDENTITY.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::IDENTITY_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: IDENTITY storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::MODEXP_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for MODEXP.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::MODEXP_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: MODEXP storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BN128ADD_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for BN128ADD.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::BN128ADD_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: BN128ADD storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BN128MUL_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for BN128MUL.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::BN128MUL_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: BN128MUL storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BN128PAIRING_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for BN128PAIRING.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::BN128PAIRING_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!(
				"precompile::AccountCodes: BN128PAIRING storage already populated. Skipping."
			)
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BLAKE2F_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for BLAKE2F.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::BLAKE2F_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: BLAKE2F storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::SHA3FIPS256_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for SHA3FIPS256.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::SHA3FIPS256_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: SHA3FIPS256 storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::DISPATCH_ADDR))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for DISPATCH.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::DISPATCH_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: DISPATCH storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(
			crate::evm::precompile::ECRECOVERPUBLICKEY_ADDR,
		))
		.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for ECRECOVERPUBLICKEY.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::ECRECOVERPUBLICKEY_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!(
				"precompile::AccountCodes: ECRECOVERPUBLICKEY storage already populated. Skipping."
			)
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::LP_AXELAR_GATEWAY))
			.is_empty()
		{
			log::info!("precompile::AccountCodes: Inserting code for LP_AXELAR_GATEWAY.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(crate::evm::precompile::LP_AXELAR_GATEWAY),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!(
				"precompile::AccountCodes: LP_AXELAR_GATEWAY storage already populated. Skipping."
			)
		}

		log::info!("precompile::AccountCodes: Inserting precompile account codes, on_runtime_upgrade: completed!");

		// NOTE: This is a worst case weight and we do not care to adjust it correctly
		// depending on skipped read/writes.
		Weight::from_ref_time(
			T::DbWeight::get()
				.read
				.ensure_mul(13)
				.unwrap_or(u64::MAX)
				.ensure_add(T::DbWeight::get().write.ensure_mul(13).unwrap_or(u64::MAX))
				.unwrap_or(u64::MAX),
		)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, &'static str> {
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::ECRECOVER_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::SHA256_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::RIPEMD160_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::IDENTITY_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::MODEXP_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BN128ADD_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BN128MUL_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::BN128PAIRING_ADDR
			)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BLAKE2F_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::SHA3FIPS256_ADDR
			)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::DISPATCH_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::ECRECOVERPUBLICKEY_ADDR
			)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::LP_AXELAR_GATEWAY
			)),
			sp_std::vec::Vec::<u8>::new()
		);

		Ok(sp_std::vec::Vec::<u8>::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), &'static str> {
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::ECRECOVER_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::SHA256_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::RIPEMD160_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::IDENTITY_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::MODEXP_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BN128ADD_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BN128MUL_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::BN128PAIRING_ADDR
			)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::BLAKE2F_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::SHA3FIPS256_ADDR
			)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(crate::evm::precompile::DISPATCH_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::ECRECOVERPUBLICKEY_ADDR
			)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(
				crate::evm::precompile::LP_AXELAR_GATEWAY
			)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);

		Ok(())
	}
}
