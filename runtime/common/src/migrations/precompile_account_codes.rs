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
use sp_core::H160;
#[cfg(feature = "try-runtime")]
use sp_runtime::DispatchError;

type Addr = [u8; 20];

const fn addr(a: u64) -> Addr {
	let b = a.to_be_bytes();
	[
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
	]
}

/// `pallet_evm::AccountCodes` must be populated for precompiles as
/// otherwise `OPCODE::EXTCODESIZE` will make the EVM error upon calling an
/// precompile.
///
/// The following bytes represent: `PUSH1 00`, `PUSH1 00`, `REVERT`.
pub const PRECOMPILE_CODE_STORAGE: [u8; 5] = hex_literal::hex!("60006000fd");

pub const ECRECOVER_ADDR: Addr = addr(0x1);
pub const SHA256_ADDR: Addr = addr(0x2);
pub const RIPEMD160_ADDR: Addr = addr(0x3);
pub const IDENTITY_ADDR: Addr = addr(0x4);
pub const MODEXP_ADDR: Addr = addr(0x5);
pub const BN128ADD_ADDR: Addr = addr(0x6);
pub const BN128MUL_ADDR: Addr = addr(0x7);
pub const BN128PAIRING_ADDR: Addr = addr(0x8);
pub const BLAKE2F_ADDR: Addr = addr(0x9);
pub const SHA3FIPS256_ADDR: Addr = addr(0x400);
pub const DISPATCH_ADDR: Addr = addr(0x401);
pub const ECRECOVERPUBLICKEY_ADDR: Addr = addr(0x402);
pub const LP_AXELAR_GATEWAY: Addr = addr(0x800);

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet_evm::Config> OnRuntimeUpgrade for Migration<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!("precompile::AccountCodes: Inserting precompile account codes: on_runtime_upgrade: started");

		if pallet_evm::AccountCodes::<T>::get(H160::from(ECRECOVER_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for ECRECOVER.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(ECRECOVER_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: ECRECOVER storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(SHA256_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for SHA256.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(SHA256_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: SHA256 storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(RIPEMD160_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for RIPEMD160.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(RIPEMD160_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: RIPEMD160 storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(IDENTITY_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for IDENTITY.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(IDENTITY_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: IDENTITY storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(MODEXP_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for MODEXP.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(MODEXP_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: MODEXP storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(BN128ADD_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for BN128ADD.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(BN128ADD_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: BN128ADD storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(BN128MUL_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for BN128MUL.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(BN128MUL_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: BN128MUL storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(BN128PAIRING_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for BN128PAIRING.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(BN128PAIRING_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!(
				"precompile::AccountCodes: BN128PAIRING storage already populated. Skipping."
			)
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(BLAKE2F_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for BLAKE2F.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(BLAKE2F_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: BLAKE2F storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(SHA3FIPS256_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for SHA3FIPS256.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(SHA3FIPS256_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: SHA3FIPS256 storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(DISPATCH_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for DISPATCH.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(DISPATCH_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!("precompile::AccountCodes: DISPATCH storage already populated. Skipping.")
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(ECRECOVERPUBLICKEY_ADDR)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for ECRECOVERPUBLICKEY.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(ECRECOVERPUBLICKEY_ADDR),
				PRECOMPILE_CODE_STORAGE.to_vec(),
			);
		} else {
			log::warn!(
				"precompile::AccountCodes: ECRECOVERPUBLICKEY storage already populated. Skipping."
			)
		}

		if pallet_evm::AccountCodes::<T>::get(H160::from(LP_AXELAR_GATEWAY)).is_empty() {
			log::info!("precompile::AccountCodes: Inserting code for LP_AXELAR_GATEWAY.");
			pallet_evm::AccountCodes::<T>::insert(
				H160::from(LP_AXELAR_GATEWAY),
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
		T::DbWeight::get().reads_writes(13, 13)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, DispatchError> {
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(ECRECOVER_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(SHA256_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(RIPEMD160_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(IDENTITY_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(MODEXP_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BN128ADD_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BN128MUL_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BN128PAIRING_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BLAKE2F_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(SHA3FIPS256_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(DISPATCH_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(ECRECOVERPUBLICKEY_ADDR)),
			sp_std::vec::Vec::<u8>::new()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(LP_AXELAR_GATEWAY)),
			sp_std::vec::Vec::<u8>::new()
		);

		Ok(sp_std::vec::Vec::<u8>::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), DispatchError> {
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(ECRECOVER_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(SHA256_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(RIPEMD160_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(IDENTITY_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(MODEXP_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BN128ADD_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BN128MUL_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BN128PAIRING_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(BLAKE2F_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(SHA3FIPS256_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(DISPATCH_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(ECRECOVERPUBLICKEY_ADDR)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);
		assert_eq!(
			pallet_evm::AccountCodes::<T>::get(H160::from(LP_AXELAR_GATEWAY)),
			PRECOMPILE_CODE_STORAGE.to_vec()
		);

		Ok(())
	}
}
