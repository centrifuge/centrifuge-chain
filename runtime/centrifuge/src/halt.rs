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

/// Pallet that permanently halts the Centrifuge runtime after the upgrade
/// block. Used to deprecate the chain.
#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	/// The block number after which the chain is permanently halted.
	/// Set by the migration during the runtime upgrade.
	#[pallet::storage]
	pub type HaltAfterBlock<T: Config> = StorageValue<_, BlockNumberFor<T>>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			if let Some(halt_after) = HaltAfterBlock::<T>::get() {
				if n > halt_after {
					panic!(
						"Centrifuge runtime has been deprecated. This chain is permanently halted."
					);
				}
			}
			T::DbWeight::get().reads(1)
		}
	}
}
