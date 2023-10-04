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
use sp_arithmetic::traits::EnsureMul;
use sp_core::H160;

use crate::evm::precompile::PRECOMPILE_CODE_STORAGE;

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet_evm::Config> OnRuntimeUpgrade for Migration<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!("💎 Inserting precompile account codes: on_runtime_upgrade: started");
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::ECRECOVER_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::SHA256_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::RIPEMD160_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::IDENTITY_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::MODEXP_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::BN128ADD_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::BN128MUL_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::BN128PAIRING_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::BLAKE2F_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::SHA3FIPS256_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::DISPATCH_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::ECRECOVERPUBLICKEY_ADDR),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);
		pallet_evm::AccountCodes::<T>::insert(
			H160::from(crate::evm::precompile::LP_AXELAR_GATEWAY),
			PRECOMPILE_CODE_STORAGE.to_vec(),
		);

		log::info!("💎 Inserting precompile account codes: on_runtime_upgrade: completed!");

		Weight::from_ref_time(T::DbWeight::get().read.ensure_mul(13).unwrap_or(u64::MAX))
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
