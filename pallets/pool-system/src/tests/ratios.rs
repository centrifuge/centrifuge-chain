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
use sp_arithmetic::{traits::EnsureSub, PerThing, Rounding};

use super::*;

#[test]
fn ensure_ratios_are_distributed_correctly_2_tranches() {
	new_test_ext().execute_with(|| {
		let pool_owner = DEFAULT_POOL_OWNER;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		util::default_pool::create();

		// Assert ratios are all zero
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(0));
			});

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(0, JuniorTrancheId::get(), 500 * CURRENCY),
				(0, SeniorTrancheId::get(), 500 * CURRENCY),
			],
		);

		// Ensure ratios are 50/50
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(50));
			});

		// Attempt to redeem 40%
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, SeniorTrancheId::get()),
			200 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, SeniorTrancheId::get()),
		));

		let new_residual_ratio = Perquintill::from_rational(5u64, 8u64);
		let mut next_ratio = new_residual_ratio;

		// Ensure ratios are 500/800 and 300/800
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, next_ratio);
				next_ratio = Perquintill::one().ensure_sub(next_ratio).unwrap();
			});

		// Attempt to redeem everything
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, SeniorTrancheId::get()),
			300 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, SeniorTrancheId::get()),
		));

		let new_residual_ratio = Perquintill::one();
		let mut next_ratio = new_residual_ratio;

		// Ensure ratios are 100/0
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, next_ratio);
				next_ratio = Perquintill::one().ensure_sub(next_ratio).unwrap();
			});

		// Ensure ratio goes up again
		invest_close_and_collect(0, vec![(0, SeniorTrancheId::get(), 300 * CURRENCY)]);
		let new_residual_ratio = Perquintill::from_rational(5u64, 8u64);
		let mut next_ratio = new_residual_ratio;

		// Ensure ratios are 500/800 and 300/800
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, next_ratio);
				next_ratio = Perquintill::one().ensure_sub(next_ratio).unwrap();
			});
	});
}

#[test]
fn ensure_ratios_are_distributed_correctly_1_tranche() {
	new_test_ext().execute_with(|| {
		let pool_owner = DEFAULT_POOL_OWNER;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		util::default_pool::create_with_tranche_input(util::default_pool::one_tranche_input());

		// Assert ratios are all zero
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(0));
			});

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(0, vec![(0, JuniorTrancheId::get(), 500 * CURRENCY)]);

		// Ensure ratios are 100
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(100));
			});

		// Attempt to redeem 40%
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
			200 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
		));

		// Ensure ratio is 100
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(100));
			});

		// Attempt to redeem everything
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
			300 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
		));

		// Ensure ratio is 0
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(0));
			});

		// Ensure ratio goes up again
		invest_close_and_collect(0, vec![(0, JuniorTrancheId::get(), 300 * CURRENCY)]);

		// Ensure ratio 100
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(100));
			});
	});
}

#[test]
fn ensure_ratios_are_distributed_correctly_3_tranches() {
	new_test_ext().execute_with(|| {
		let pool_owner = DEFAULT_POOL_OWNER;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		util::default_pool::create_with_tranche_input(util::default_pool::three_tranche_input());

		// Assert ratios are all zero
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::from_percent(0));
			});

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(0, JuniorTrancheId::get(), 500 * CURRENCY),
				(0, SeniorTrancheId::get(), 500 * CURRENCY),
				(0, SecondSeniorTrancheId::get(), 500 * CURRENCY),
			],
		);

		let check_ratios = [
			Perquintill::from_rational_with_rounding(1u64, 3, Rounding::Up).unwrap(),
			Perquintill::from_rational(1u64, 3),
			Perquintill::from_rational(1u64, 3),
		];

		// Ensure ratios are 100
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.zip(check_ratios.into_iter())
			.for_each(|(tranche, check)| {
				assert_eq!(tranche.ratio, check);
			});

		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
			250 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
		));

		let check_ratios = [
			Perquintill::from_rational(1u64, 5),
			Perquintill::from_rational(2u64, 5),
			Perquintill::from_rational(2u64, 5),
		];

		// Ensure ratios are 100
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.zip(check_ratios.into_iter())
			.for_each(|(tranche, check)| {
				assert_eq!(tranche.ratio, check);
			});

		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, SecondSeniorTrancheId::get()),
			250 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, SecondSeniorTrancheId::get()),
		));

		let check_ratios = [
			Perquintill::from_rational(1u64, 4),
			Perquintill::from_rational(2u64, 4),
			Perquintill::from_rational(1u64, 4),
		];

		// Ensure ratios are 100
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.zip(check_ratios.into_iter())
			.for_each(|(tranche, check)| {
				assert_eq!(tranche.ratio, check);
			});

		// Ensure ratio goes up again
		invest_close_and_collect(0, vec![(0, JuniorTrancheId::get(), 250 * CURRENCY)]);

		let check_ratios = [
			Perquintill::from_rational(2u64, 5),
			Perquintill::from_rational(2u64, 5),
			Perquintill::from_rational(1u64, 5),
		];

		// Ensure ratios are 100
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.zip(check_ratios.into_iter())
			.for_each(|(tranche, check)| {
				assert_eq!(tranche.ratio, check);
			});

		// Redeem everything
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, SecondSeniorTrancheId::get()),
			250 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, SecondSeniorTrancheId::get()),
		));

		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, SeniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, SeniorTrancheId::get()),
		));

		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			(0, JuniorTrancheId::get()),
		));

		// Ensure ratios are 0
		Pool::<Runtime>::get(0)
			.unwrap()
			.tranches
			.residual_top_slice()
			.iter()
			.for_each(|tranche| {
				assert_eq!(tranche.ratio, Perquintill::zero());
			});
	});
}
