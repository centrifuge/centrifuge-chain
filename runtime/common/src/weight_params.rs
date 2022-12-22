// Copyright 2022 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

/// Generates Weight params (Get impl) for Runtimes with the max proof size pulled from the relay if in an externalities provided
/// environment, and the max proof size for relay chains as defined by polkadot used if not.
/// Provides:
/// - MaximumBlockWeight: MAXIMIM_BLOCK_WEIGHT with proof size adjusted for relay chain val.
/// - BlockWeightsWithRelayProof: BlockWeights generated with using MaximumBlockWeight with relay proof size set.
/// - MessagingReservedWeight: chain messaging reserved weight using MaximumBlockWeight with relay proof size set.
/// - MaximumSchedulerWeight: max scheduler weight using MaximumBlockWeight with relay proof size set.

#[macro_export]
macro_rules! gen_weight_parameters {
	() => {
		//  Currently using a Macro as seperating these out into a seperate module usable by the different runtimes
		// causes issues with the `__construct_runtime_integrity_test::runtime_integrity_tests` test for runtime/development
		// with the cfg!(test) not returning correctly for seperate module, and then attempted data fetch relay chain with no
		// externalities provided... which panics.  Works for tests with provided externalities env & execute_with

		// Curently have Runtime generic as we would need this to switch back from macro, though we could technically
		// remove now, though the impl where does provide context for which config would need to be impl for runtime
		// to fetch the validation data for the relay max proof size.

		/// Struct for Get impl of MaximumBlockWeight with Weight using relay max_pov_size as proof size.
		pub struct MaximumBlockWeight<Runtime>(sp_std::marker::PhantomData<Runtime>);

		impl<Runtime> sp_core::Get<frame_support::weights::Weight> for MaximumBlockWeight<Runtime>
		where
			Runtime: cumulus_pallet_parachain_system::Config,
		{
			fn get() -> frame_support::weights::Weight {
				if cfg!(test) {
					cfg_primitives::constants::MAXIMUM_BLOCK_WEIGHT
				} else {
					let max_pov_size =
						cumulus_pallet_parachain_system::Pallet::<Runtime>::validation_data()
							.map(|x| x.max_pov_size)
							.unwrap_or(cumulus_primitives_core::relay_chain::v2::MAX_POV_SIZE);
					cfg_primitives::constants::MAXIMUM_BLOCK_WEIGHT
						.set_proof_size(max_pov_size.into())
						.into()
				}
			}
		}

		/// Struct for Get impl of BlockWeights with BlockWeight generation using MaximumBlockWeight with relay proof size set.
		pub struct BlockWeightsWithRelayProof<Runtime>(sp_std::marker::PhantomData<Runtime>);

		impl<Runtime> sp_core::Get<BlockWeights> for BlockWeightsWithRelayProof<Runtime>
		where
			Runtime: cumulus_pallet_parachain_system::Config,
		{
			fn get() -> frame_system::limits::BlockWeights {
				let max_weight = MaximumBlockWeight::<Runtime>::get();

				BlockWeights::builder()
					.base_block(frame_support::weights::constants::BlockExecutionWeight::get())
					.for_class(DispatchClass::all(), |weights| {
						weights.base_extrinsic =
							frame_support::weights::constants::ExtrinsicBaseWeight::get();
					})
					.for_class(frame_support::dispatch::DispatchClass::Normal, |weights| {
						weights.max_total =
							Some(cfg_primitives::constants::NORMAL_DISPATCH_RATIO * max_weight);
					})
					.for_class(
						frame_support::dispatch::DispatchClass::Operational,
						|weights| {
							weights.max_total = Some(max_weight);
							// Operational transactions have some extra reserved space, so that they
							// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
							weights.reserved = Some(
								max_weight
									- cfg_primitives::constants::NORMAL_DISPATCH_RATIO * max_weight,
							);
						},
					)
					.avg_block_initialization(
						cfg_primitives::constants::AVERAGE_ON_INITIALIZE_RATIO,
					)
					// NOTE: We could think about chaning this to something that is sane default with a
					//       error log. As we now depend on some dynamic state from the relay-chain
					.build_or_panic()
			}
		}

		/// Struct for Get impl of MessageingReservedWeight Weight generation using MaximumBlockWeight with relay proof size set.
		pub struct MessagingReservedWeight<Runtime>(sp_std::marker::PhantomData<Runtime>);

		impl<Runtime> sp_core::Get<frame_support::weights::Weight>
			for MessagingReservedWeight<Runtime>
		where
			Runtime: cumulus_pallet_parachain_system::Config,
		{
			fn get() -> Weight {
				MaximumBlockWeight::<Runtime>::get().saturating_div(4)
			}
		}

		/// Struct for Get impl of MaximumSchedulerWeight Weight generation using MaximumBlockWeight with relay proof set.
		pub struct MaximumSchedulerWeight<Runtime>(sp_std::marker::PhantomData<Runtime>);
		impl<Runtime> sp_core::Get<frame_support::weights::Weight>
			for MaximumSchedulerWeight<Runtime>
		where
			Runtime: cumulus_pallet_parachain_system::Config,
		{
			fn get() -> Weight {
				(sp_runtime::Perbill::from_percent(80) * MaximumBlockWeight::<Runtime>::get())
					.into()
			}
		}
	};
}
