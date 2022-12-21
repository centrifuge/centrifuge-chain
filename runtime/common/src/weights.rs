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

use cfg_primitives::constants::{
	AVERAGE_ON_INITIALIZE_RATIO, MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO,
};
use cumulus_primitives_core::relay_chain::v2::MAX_POV_SIZE;
use frame_support::{
	dispatch::DispatchClass,
	sp_std::marker::PhantomData,
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight},
		Weight,
	},
};
use frame_system::limits::BlockWeights;
use sp_arithmetic::Perbill;
use sp_core::Get;

/// Strut for Get impl of BlockWeights with BlockWeight generation with relay max_pov_size as proof size
pub struct BlockWeightsWithRelayProof<Runtime>(PhantomData<Runtime>);

impl<Runtime> Get<BlockWeights> for BlockWeightsWithRelayProof<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn get() -> BlockWeights {
		let max_weight = MaximumBlockWeight::<Runtime>::get();

		BlockWeights::builder()
			.base_block(BlockExecutionWeight::get())
			.for_class(DispatchClass::all(), |weights| {
				weights.base_extrinsic = ExtrinsicBaseWeight::get();
			})
			.for_class(DispatchClass::Normal, |weights| {
				weights.max_total = Some(NORMAL_DISPATCH_RATIO * max_weight);
			})
			.for_class(DispatchClass::Operational, |weights| {
				weights.max_total = Some(max_weight);
				// Operational transactions have some extra reserved space, so that they
				// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
				weights.reserved = Some(max_weight - NORMAL_DISPATCH_RATIO * max_weight);
			})
			.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
			// NOTE: We could think about chaning this to something that is sane default with a
			//       error log. As we now depend on some dynamic state from the relay-chain
			.build_or_panic()
	}
}

/// Strut for Get impl of MaximumBlockWeight with Weight using relay max_pov_size as proof size
pub struct MaximumBlockWeight<Runtime>(PhantomData<Runtime>);

impl<Runtime> Get<Weight> for MaximumBlockWeight<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn get() -> Weight {
		if cfg!(test) {
			MAXIMUM_BLOCK_WEIGHT
		} else {
			let max_pov_size =
				cumulus_pallet_parachain_system::Pallet::<Runtime>::validation_data()
					.map(|x| x.max_pov_size)
					.unwrap_or(MAX_POV_SIZE);
			MAXIMUM_BLOCK_WEIGHT
				.set_proof_size(max_pov_size.into())
				.into()
		}
	}
}

pub struct MessagingReservedWeight<Runtime>(sp_std::marker::PhantomData<Runtime>);

impl<Runtime> Get<Weight> for MessagingReservedWeight<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn get() -> Weight {
		MaximumBlockWeight::<Runtime>::get().saturating_div(4)
	}
}

pub struct MaximumSchedulerWeight<Runtime>(sp_std::marker::PhantomData<Runtime>);
impl<Runtime> Get<Weight> for MaximumSchedulerWeight<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn get() -> Weight {
		(Perbill::from_percent(80) * MaximumBlockWeight::<Runtime>::get()).into()
	}
}
