// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

//! OpenGov Tracks parametrization
//!
//! Based on [Centrifuge Chain parametrization](https://docs.google.com/document/d/1aPH1sTfAttD3K0f4AoOF2iDb3nH1GHlx5TfIP4Xi4SM).

use cfg_primitives::{
	Balance, BlockNumber, CFG, DAYS, HOURS, MINUTES, TRACK_INDEX_POOL_ADMIN,
	TRACK_INDEX_REF_CANCELLER, TRACK_INDEX_REF_KILLER, TRACK_INDEX_ROOT, TRACK_INDEX_TREASURER,
	TRACK_INDEX_WHITELISTED_CALLER,
};
use pallet_referenda::Curve;
use runtime_common::{to_percent, to_ppm};

use crate::RuntimeOrigin;

const SUP_ROOT: Curve =
	Curve::make_reciprocal(1, 14, to_percent(10), to_ppm(8800), to_percent(100));
const APP_ROOT: Curve = Curve::make_linear(14, 14, to_percent(50), to_percent(100));

const SUP_WHITELISTED: Curve =
	Curve::make_reciprocal(1, 7, to_ppm(800), to_ppm(100), to_percent(50));
const APP_WHITELISTED: Curve = Curve::make_linear(7, 7, to_percent(50), to_percent(100));

const SUP_POOL_ADMIN: Curve =
	Curve::make_reciprocal(1, 7, to_ppm(38500), to_ppm(5900), to_percent(50));
const APP_POOL_ADMIN: Curve = Curve::make_linear(7, 7, to_percent(70), to_percent(100));

const SUP_TREASURER: Curve = Curve::make_linear(25, 28, to_percent(1), to_percent(50));
const APP_TREASURER: Curve = Curve::make_linear(14, 14, to_percent(70), to_percent(100));

const SUP_REF_CANCELLER: Curve =
	Curve::make_reciprocal(1, 7, to_ppm(38500), to_ppm(5900), to_percent(50));
const APP_REF_CANCELLER: Curve = Curve::make_linear(7, 7, to_percent(70), to_percent(100));

const SUP_REF_KILLER: Curve =
	Curve::make_reciprocal(1, 7, to_ppm(38500), to_ppm(5900), to_percent(50));
const APP_REF_KILLER: Curve = Curve::make_linear(7, 7, to_percent(70), to_percent(100));

const TRACKS_DATA: [(u16, pallet_referenda::TrackInfo<Balance, BlockNumber>); 6] = [
	(
		TRACK_INDEX_ROOT,
		pallet_referenda::TrackInfo {
			name: "root",
			max_deciding: 2,
			decision_deposit: 300_000 * CFG,
			prepare_period: 6 * HOURS,
			decision_period: 14 * DAYS,
			confirm_period: 12 * HOURS,
			min_enactment_period: 24 * HOURS,
			min_approval: APP_ROOT,
			min_support: SUP_ROOT,
		},
	),
	(
		TRACK_INDEX_WHITELISTED_CALLER,
		pallet_referenda::TrackInfo {
			name: "whitelisted_caller",
			max_deciding: 20,
			decision_deposit: 1_000 * CFG,
			prepare_period: 10 * MINUTES,
			decision_period: 7 * DAYS,
			confirm_period: 10 * MINUTES,
			min_enactment_period: 10 * MINUTES,
			min_approval: APP_WHITELISTED,
			min_support: SUP_WHITELISTED,
		},
	),
	(
		TRACK_INDEX_POOL_ADMIN,
		pallet_referenda::TrackInfo {
			name: "pool_admin",
			max_deciding: 5,
			decision_deposit: 1_000 * CFG,
			prepare_period: 1 * HOURS,
			decision_period: 7 * DAYS,
			confirm_period: 1 * HOURS,
			min_enactment_period: 1 * HOURS,
			min_approval: APP_POOL_ADMIN,
			min_support: SUP_POOL_ADMIN,
		},
	),
	(
		TRACK_INDEX_TREASURER,
		pallet_referenda::TrackInfo {
			name: "treasurer",
			max_deciding: 2,
			decision_deposit: 10_000 * CFG,
			prepare_period: 6 * HOURS,
			decision_period: 14 * DAYS,
			confirm_period: 12 * HOURS,
			min_enactment_period: 12 * HOURS,
			min_approval: APP_TREASURER,
			min_support: SUP_TREASURER,
		},
	),
	(
		TRACK_INDEX_REF_CANCELLER,
		pallet_referenda::TrackInfo {
			name: "referendum_canceller",
			max_deciding: 20,
			decision_deposit: 50_000 * CFG,
			prepare_period: 1 * HOURS,
			decision_period: 7 * DAYS,
			confirm_period: 1 * HOURS,
			min_enactment_period: 10 * MINUTES,
			min_approval: APP_REF_CANCELLER,
			min_support: SUP_REF_CANCELLER,
		},
	),
	(
		TRACK_INDEX_REF_KILLER,
		pallet_referenda::TrackInfo {
			name: "referendum_killer",
			max_deciding: 20,
			decision_deposit: 75_000 * CFG,
			prepare_period: 1 * HOURS,
			decision_period: 7 * DAYS,
			confirm_period: 1 * HOURS,
			min_enactment_period: 10 * MINUTES,
			min_approval: APP_REF_KILLER,
			min_support: SUP_REF_KILLER,
		},
	),
];

pub struct TracksInfo;
impl pallet_referenda::TracksInfo<Balance, BlockNumber> for TracksInfo {
	type Id = u16;
	type RuntimeOrigin = <RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin;

	fn tracks() -> &'static [(Self::Id, pallet_referenda::TrackInfo<Balance, BlockNumber>)] {
		&TRACKS_DATA[..]
	}

	fn track_for(id: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
		if let Ok(system_origin) = frame_system::RawOrigin::try_from(id.clone()) {
			match system_origin {
				frame_system::RawOrigin::Root => Ok(TRACK_INDEX_ROOT),
				_ => Err(()),
			}
		} else if let Ok(custom_origin) = runtime_common::origins::gov::Origin::try_from(id.clone())
		{
			match custom_origin {
				runtime_common::origins::gov::Origin::WhitelistedCaller => {
					Ok(TRACK_INDEX_WHITELISTED_CALLER)
				}
				// General admin
				runtime_common::origins::gov::Origin::PoolAdmin => Ok(TRACK_INDEX_POOL_ADMIN),
				runtime_common::origins::gov::Origin::Treasurer => Ok(TRACK_INDEX_TREASURER),
				// Referendum admins
				runtime_common::origins::gov::Origin::ReferendumCanceller => {
					Ok(TRACK_INDEX_REF_CANCELLER)
				}
				runtime_common::origins::gov::Origin::ReferendumKiller => {
					Ok(TRACK_INDEX_REF_KILLER)
				}
			}
		} else {
			Err(())
		}
	}
}
pallet_referenda::impl_tracksinfo_get!(TracksInfo, Balance, BlockNumber);

#[cfg(test)]
mod tests {
	use sp_arithmetic::Perbill;

	use super::*;

	const PRECISION: u32 = 10000;

	fn round(x: Perbill) -> Perbill {
		Perbill::from_rational(x * PRECISION, PRECISION)
	}

	#[test]
	fn root_track() {
		todo!()
	}

	#[test]
	fn whitelisted_caller_track() {
		todo!()
	}
	#[test]
	fn pool_admin_track() {
		todo!()
	}

	#[test]
	fn treasurer_track() {
		const HOURS: [u32; 8] = [0, 1, 2, 3, 24, 300, 324, 336];
		let approval: [Perbill; 8] = [
			Perbill::from_rational(100, 100u32),
			Perbill::from_rational(9991, 10000u32),
			Perbill::from_rational(9982, 10000u32),
			Perbill::from_rational(9973, 10000u32),
			Perbill::from_rational(9786, 10000u32),
			Perbill::from_rational(7321, 10000u32),
			Perbill::from_rational(7107, 10000u32),
			Perbill::from_rational(70, 100u32),
		];
		let sup: [Perbill; 8] = [
			Perbill::from_rational(50, 100u32),
			Perbill::from_rational(4984, 10000u32),
			Perbill::from_rational(4967, 10000u32),
			Perbill::from_rational(4951, 10000u32),
			Perbill::from_rational(4608, 10000u32),
			Perbill::from_rational(1, 100u32),
			Perbill::from_rational(1, 100u32),
			Perbill::from_rational(1, 100u32),
		];

		for (hour, y) in HOURS.into_iter().zip(approval) {
			assert_eq!(
				round(APP_TREASURER.threshold(Perbill::from_rational(hour, 14u32 * 24u32))),
				y,
				"Approval mismatch at hour {hour}"
			);
		}

		for (hour, y) in HOURS.into_iter().zip(sup) {
			assert_eq!(
				round(SUP_TREASURER.threshold(Perbill::from_rational(hour, 14u32 * 24u32))),
				y,
				"Support mismatch at hour {hour}"
			);
		}
	}

	#[test]
	fn ref_canceller_track() {
		const HOURS: [u32; 8] = [0, 1, 2, 3, 24, 72, 120, 168];
		let approval: [Perbill; 8] = [
			Perbill::from_rational(100, 100u32),
			Perbill::from_rational(9982, 10000u32),
			Perbill::from_rational(9964, 10000u32),
			Perbill::from_rational(9946, 10000u32),
			Perbill::from_rational(9571, 10000u32),
			Perbill::from_rational(8714, 10000u32),
			Perbill::from_rational(7857, 10000u32),
			Perbill::from_rational(70, 100u32),
		];
		let sup: [Perbill; 8] = [
			Perbill::from_rational(50, 100u32),
			Perbill::from_rational(3333, 10000u32),
			Perbill::from_rational(25, 100u32),
			Perbill::from_rational(20, 100u32),
			Perbill::from_rational(385, 10000u32),
			Perbill::from_rational(135, 10000u32),
			Perbill::from_rational(82, 10000u32),
			Perbill::from_rational(59, 10000u32),
		];

		for (hour, y) in HOURS.into_iter().zip(approval) {
			assert_eq!(
				round(APP_REF_CANCELLER.threshold(Perbill::from_rational(hour, 7u32 * 24u32))),
				y,
				"Approval mismatch at hour {hour}"
			);
		}

		for (hour, y) in HOURS.into_iter().zip(sup) {
			assert_eq!(
				round(SUP_REF_CANCELLER.threshold(Perbill::from_rational(hour, 7u32 * 24u32))),
				y,
				"Support mismatch at hour {hour}"
			);
		}
	}

	#[test]
	fn ref_killer_track() {
		const HOURS: [u32; 8] = [0, 1, 2, 3, 24, 72, 120, 168];
		let approval: [Perbill; 8] = [
			Perbill::from_rational(100, 100u32),
			Perbill::from_rational(9982, 10000u32),
			Perbill::from_rational(9964, 10000u32),
			Perbill::from_rational(9946, 10000u32),
			Perbill::from_rational(9571, 10000u32),
			Perbill::from_rational(8714, 10000u32),
			Perbill::from_rational(7857, 10000u32),
			Perbill::from_rational(70, 100u32),
		];
		let sup: [Perbill; 8] = [
			Perbill::from_rational(50, 100u32),
			Perbill::from_rational(3333, 10000u32),
			Perbill::from_rational(25, 100u32),
			Perbill::from_rational(20, 100u32),
			Perbill::from_rational(385, 10000u32),
			Perbill::from_rational(135, 10000u32),
			Perbill::from_rational(82, 10000u32),
			Perbill::from_rational(59, 10000u32),
		];

		for (hour, y) in HOURS.into_iter().zip(approval) {
			assert_eq!(
				round(APP_REF_KILLER.threshold(Perbill::from_rational(hour, 7u32 * 24u32))),
				y,
				"Approval mismatch at hour {hour}"
			);
		}

		for (hour, y) in HOURS.into_iter().zip(sup) {
			assert_eq!(
				round(SUP_REF_KILLER.threshold(Perbill::from_rational(hour, 7u32 * 24u32))),
				y,
				"Support mismatch at hour {hour}"
			);
		}
	}
}
