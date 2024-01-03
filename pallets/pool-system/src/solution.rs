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

use frame_support::sp_runtime::traits::Convert;
use parity_scale_codec::MaxEncodedLen;
use sp_arithmetic::traits::Unsigned;
use sp_runtime::{
	traits::{EnsureFixedPointNumber, EnsureSub},
	ArithmeticError,
};
use sp_std::{ops::Deref, vec};

use super::*;
use crate::tranches::{calculate_risk_buffers, EpochExecutionTranches, TrancheSolution};

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum PoolState {
	Healthy,
	Unhealthy(Vec<UnhealthyState>),
}

impl PoolState {
	/// Updates a PoolState to update.
	///
	/// NOTE:
	/// * This will switch a PoolState::Healthy -> PoolState::Unhealthy(_) and
	///   vice versa
	/// * If an already unhealthy state is updated, the new
	///   `Vec<UnhealthyState>` inside the enum will be **overwritten** with the
	///   newly passed unhealthy states. -> Use `add_unhealthy` or
	///   `rm_unhealthy` if the other states should be kept.
	pub fn update(&mut self, update: PoolState) -> &mut Self {
		*self = update;
		self
	}

	/// Adds an unhealthy state
	///
	/// * If the state was not present yet, it will be added to the
	/// vector of unhealthy states. If it was added, then it will
	/// not be added a second time.
	///
	/// * If the state was previously healthy, then this puts the
	/// state into an unhealthy state!
	pub fn add_unhealthy(&mut self, add: UnhealthyState) -> &mut Self {
		match self {
			PoolState::Healthy => {
				let states = vec![add];
				*self = PoolState::Unhealthy(states);
				self
			}
			PoolState::Unhealthy(states) => {
				if !states.contains(&add) {
					states.push(add);
				}
				self
			}
		}
	}

	/// Removes an unhealthy state
	///
	/// * If the state was not present yet, it will not be removed,
	///  it it was present if will be removed.
	///
	/// * If there are no more unhealthy states, the state will
	/// be switched to healthy
	///
	/// * If the state was healthy, this is a no-op
	pub fn rm_unhealthy(&mut self, rm: UnhealthyState) -> &mut Self {
		match self {
			PoolState::Healthy => self,
			PoolState::Unhealthy(states) => {
				states.retain(|val| val != &rm);

				if states.is_empty() {
					*self = PoolState::Healthy;
				}
				self
			}
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum UnhealthyState {
	MaxReserveViolated,
	MinRiskBufferViolated,
}

/// The solutions struct for epoch solution
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum EpochSolution<Balance, MaxTranches>
where
	MaxTranches: Get<u32>,
{
	Healthy(HealthySolution<Balance, MaxTranches>),
	Unhealthy(UnhealthySolution<Balance, MaxTranches>),
}

/// The information for a currently executing epoch
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EpochExecutionInfo<
	Balance,
	BalanceRatio,
	EpochId,
	Weight,
	BlockNumber,
	TrancheCurrency,
	MaxTranches,
> where
	MaxTranches: Get<u32>,
{
	pub epoch: EpochId,
	pub nav: Balance,
	pub reserve: Balance,
	pub max_reserve: Balance,
	pub tranches:
		EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency, MaxTranches>,
	pub best_submission: Option<EpochSolution<Balance, MaxTranches>>,
	pub challenge_period_end: Option<BlockNumber>,
}

impl<Balance, MaxTranches> EpochSolution<Balance, MaxTranches>
where
	MaxTranches: Get<u32>,
{
	/// Calculates the score for a given solution. Should only be called inside
	/// the `fn score_solution()` from the runtime, as there are no checks if
	/// solution length matches tranche length.
	///
	/// Scores are calculated with the following function
	///
	/// Notation:
	///  * X(a) -> A vector of a's, where each element is associated with a
	///    tranche
	///  * ||X(a)||1 -> 1-Norm of a vector, i.e. the absolute sum over all
	///    elements
	///
	///  X = X(%-invest-fulfillments) * X(investments) *
	/// X(invest_tranche_weights)            + X(%-redeem-fulfillments) *
	/// X(redemptions) * X(redeem_tranche_weights)
	///
	///  score = ||X||1
	///
	/// Returns error upon overflow of `Balances`.
	pub fn calculate_score<BalanceRatio, Weight, TrancheCurrency, MaxExecutionTranches>(
		solution: &[TrancheSolution],
		tranches: &EpochExecutionTranches<
			Balance,
			BalanceRatio,
			Weight,
			TrancheCurrency,
			MaxExecutionTranches,
		>,
	) -> Result<Balance, DispatchError>
	where
		Balance: Copy + BaseArithmetic + Unsigned + From<u64>,
		Weight: Copy + From<u128> + Convert<Weight, Balance>,
		BalanceRatio: Copy,
		MaxExecutionTranches: Get<u32>,
	{
		let (invest_score, redeem_score) = solution
			.iter()
			.zip(tranches.residual_top_slice())
			.zip(tranches.calculate_weights())
			.try_fold(
				(Balance::zero(), Balance::zero()),
				|(invest_score, redeem_score),
				 ((solution, tranches), (invest_weight, redeem_weight))|
				 -> Result<_, DispatchError> {
					Ok((
						solution
							.invest_fulfillment
							.mul_floor(tranches.invest)
							.ensure_mul(Weight::convert(invest_weight))?
							.ensure_add(invest_score)?,
						solution
							.redeem_fulfillment
							.mul_floor(tranches.redeem)
							.ensure_mul(Weight::convert(redeem_weight))?
							.ensure_add(redeem_score)?,
					))
				},
			)?;

		Ok(invest_score.ensure_add(redeem_score)?)
	}

	/// Scores a solution and returns a healthy solution as a result.
	pub fn score_solution_healthy<BalanceRatio, Weight, TrancheCurrency, MaxExecutionTranches>(
		solution: &[TrancheSolution],
		tranches: &EpochExecutionTranches<
			Balance,
			BalanceRatio,
			Weight,
			TrancheCurrency,
			MaxExecutionTranches,
		>,
	) -> Result<EpochSolution<Balance, MaxTranches>, DispatchError>
	where
		Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
		Weight: Copy + From<u128> + Convert<Weight, Balance>,
		BalanceRatio: Copy,
		MaxExecutionTranches: Get<u32>,
	{
		let score = Self::calculate_score(solution, tranches)?;

		Ok(EpochSolution::Healthy(HealthySolution {
			solution: BoundedVec::truncate_from(solution.to_vec()),
			score,
		}))
	}

	/// Scores an solution, that would bring a pool into an unhealthy state.
	pub fn score_solution_unhealthy<BalanceRatio, Weight, TrancheCurrency, MaxExecutionTranches>(
		solution: &[TrancheSolution],
		tranches: &EpochExecutionTranches<
			Balance,
			BalanceRatio,
			Weight,
			TrancheCurrency,
			MaxExecutionTranches,
		>,
		reserve: Balance,
		max_reserve: Balance,
		state: &[UnhealthyState],
	) -> Result<EpochSolution<Balance, MaxTranches>, DispatchError>
	where
		Weight: Copy + From<u128>,
		BalanceRatio: Copy + FixedPointNumber,
		Balance: Copy
			+ BaseArithmetic
			+ FixedPointOperand
			+ Unsigned
			+ From<u64>
			+ sp_arithmetic::MultiplyRational,
		MaxExecutionTranches: Get<u32>,
	{
		let risk_buffer_improvement_scores =
			if state.contains(&UnhealthyState::MinRiskBufferViolated) {
				let risk_buffers = calculate_risk_buffers(
					&tranches.supplies_with_fulfillment(solution)?,
					&tranches.prices(),
				)?;

				// Score: 1 / (min risk buffer - risk buffer)
				// A higher score means the distance to the min risk buffer is smaller
				let non_junior_tranches =
					tranches
						.non_residual_tranches()
						.ok_or(DispatchError::Other(
							"Corrupted PoolState. Getting NonResidualTranches infailable.",
						))?;
				Some(
					non_junior_tranches
						.iter()
						.zip(risk_buffers)
						.map(|(tranche, risk_buffer)| {
							Ok(tranche
								.min_risk_buffer
								.ensure_sub(risk_buffer)?
								.saturating_reciprocal_mul(Balance::one()))
						})
						.collect::<Result<Vec<_>, ArithmeticError>>()?,
				)
			} else {
				None
			};

		let reserve_improvement_score = if state.contains(&UnhealthyState::MaxReserveViolated) {
			let mut acc_invest = Balance::zero();
			let mut acc_redeem = Balance::zero();
			tranches.combine_with_residual_top(solution, |tranche, solution| {
				acc_invest
					.ensure_add_assign(solution.invest_fulfillment.mul_floor(tranche.invest))?;

				acc_redeem
					.ensure_add_assign(solution.redeem_fulfillment.mul_floor(tranche.redeem))?;

				Ok(())
			})?;

			let new_reserve = reserve.ensure_add(acc_invest)?.ensure_sub(acc_redeem)?;

			// Score: 1 / (new reserve - max reserve)
			// A higher score means the distance to the max reserve is smaller
			let reserve_diff = new_reserve.ensure_sub(max_reserve)?;
			let score = BalanceRatio::one().ensure_div_int(reserve_diff)?;

			Some(score)
		} else {
			None
		};

		Ok(EpochSolution::Unhealthy(UnhealthySolution {
			state: BoundedVec::truncate_from(state.to_vec()),
			solution: BoundedVec::truncate_from(solution.to_vec()),
			risk_buffer_improvement_scores: risk_buffer_improvement_scores
				.map(|v| BoundedVec::truncate_from(v)),
			reserve_improvement_score,
		}))
	}
}

impl<Balance, MaxTranches> EpochSolution<Balance, MaxTranches>
where
	Balance: Copy,
	MaxTranches: Get<u32>,
{
	pub fn healthy(&self) -> bool {
		match self {
			EpochSolution::Healthy(_) => true,
			EpochSolution::Unhealthy(_) => false,
		}
	}

	pub fn solution(&self) -> &[TrancheSolution] {
		match self {
			EpochSolution::Healthy(solution) => solution.solution.as_slice(),
			EpochSolution::Unhealthy(solution) => solution.solution.as_slice(),
		}
	}
}

impl<Balance, MaxTranches> PartialOrd for EpochSolution<Balance, MaxTranches>
where
	Balance: PartialOrd,
	MaxTranches: Get<u32> + PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		match (self, other) {
			(EpochSolution::Healthy(s_1), EpochSolution::Healthy(s_2)) => s_1.partial_cmp(s_2),
			(EpochSolution::Healthy(_), EpochSolution::Unhealthy(_)) => Some(Ordering::Greater),
			(EpochSolution::Unhealthy(s_1), EpochSolution::Unhealthy(s_2)) => s_1.partial_cmp(s_2),
			(EpochSolution::Unhealthy(_), EpochSolution::Healthy(_)) => Some(Ordering::Less),
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct HealthySolution<Balance, MaxTranches: Get<u32>> {
	// TODO: Check depedency of Tranches, Solutions and States. E.g. can we use the same max bounds
	// for multiple different bounded vecs?
	pub solution: BoundedVec<TrancheSolution, MaxTranches>,
	pub score: Balance,
}

impl<Balance, MaxTranches> PartialOrd for HealthySolution<Balance, MaxTranches>
where
	Balance: PartialOrd,
	MaxTranches: Get<u32> + PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.score.partial_cmp(&other.score)
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct UnhealthySolution<Balance, MaxTranches: Get<u32>> {
	// TODO: Check depedency of Tranches, Solutions and States. E.g. can we use the same max bounds
	// for multiple different bounded vecs?
	pub state: BoundedVec<UnhealthyState, MaxTranches>,
	pub solution: BoundedVec<TrancheSolution, MaxTranches>,
	// The risk buffer score per tranche (less junior tranche) for this solution
	pub risk_buffer_improvement_scores: Option<BoundedVec<Balance, MaxTranches>>,
	// The reserve buffer score for this solution
	pub reserve_improvement_score: Option<Balance>,
}

impl<Balance, MaxTranches> UnhealthySolution<Balance, MaxTranches>
where
	MaxTranches: Get<u32>,
{
	fn has_state(&self, state: &UnhealthyState) -> bool {
		self.state.deref().contains(state)
	}
}

impl<Balance, MaxTranches> PartialOrd for UnhealthySolution<Balance, MaxTranches>
where
	Balance: PartialOrd,
	MaxTranches: Get<u32> + PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		// We check if any of the risk buffer scores are higher.
		// A higher risk buffer score for a more senior tranche is more important
		// than one for a less senior tranche.
		match (
			self.has_state(&UnhealthyState::MinRiskBufferViolated),
			other.has_state(&UnhealthyState::MinRiskBufferViolated),
		) {
			(true, true) => {
				// If both vectors exist, we compare them element by element.
				// As we sort tranches from senior -> junior, we ensure, that
				// at the moment where a solution improves the risk buffer of a more
				// senior tranche, it is ruled the better solution!
				if self.risk_buffer_improvement_scores > other.risk_buffer_improvement_scores {
					return Some(Ordering::Greater);
				} else if self.risk_buffer_improvement_scores < other.risk_buffer_improvement_scores
				{
					return Some(Ordering::Less);
				}
			}
			(false, true) => return Some(Ordering::Greater),
			(true, false) => return Some(Ordering::Less),
			(false, false) => (),
		}

		// If there are no differences in risk buffer scores or there is no risk buffer
		// violation we look at the reserve improvement score.
		match (
			self.has_state(&UnhealthyState::MaxReserveViolated),
			other.has_state(&UnhealthyState::MaxReserveViolated),
		) {
			(true, true) => {
				if self.reserve_improvement_score > other.reserve_improvement_score {
					return Some(Ordering::Greater);
				} else if self.reserve_improvement_score < other.reserve_improvement_score {
					return Some(Ordering::Less);
				}
			}
			(false, true) => return Some(Ordering::Greater),
			(true, false) => return Some(Ordering::Less),
			(false, false) => (),
		}

		// If both of the above rules to not apply, we value the solutions as equal
		Some(Ordering::Equal)
	}
}

pub fn calculate_solution_parameters<Balance, BalanceRatio, Rate, Weight, Currency, MaxTranches>(
	epoch_tranches: &EpochExecutionTranches<Balance, BalanceRatio, Weight, Currency, MaxTranches>,
	solution: &[TrancheSolution],
) -> Result<(Balance, Balance, Vec<Perquintill>), DispatchError>
where
	BalanceRatio: Copy + FixedPointNumber,
	Balance: Copy
		+ BaseArithmetic
		+ FixedPointOperand
		+ Unsigned
		+ From<u64>
		+ sp_arithmetic::MultiplyRational,
	Weight: Copy + From<u128>,
	MaxTranches: Get<u32>,
{
	let acc_invest: Balance = epoch_tranches
		.residual_top_slice()
		.iter()
		.zip(solution)
		.try_fold(Balance::zero(), |sum, (tranche, solution)| {
			sum.ensure_add(solution.invest_fulfillment.mul_floor(tranche.invest))
		})?;

	let acc_redeem: Balance = epoch_tranches
		.residual_top_slice()
		.iter()
		.zip(solution)
		.try_fold(Balance::zero(), |sum, (tranche, solution)| {
			sum.ensure_add(solution.redeem_fulfillment.mul_floor(tranche.redeem))
		})?;

	let new_tranche_supplies = epoch_tranches.supplies_with_fulfillment(solution)?;
	let tranche_prices = epoch_tranches.prices();
	let risk_buffers = calculate_risk_buffers(&new_tranche_supplies, &tranche_prices)?;

	Ok((acc_invest, acc_redeem, risk_buffers))
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::mock::MaxTranches;

	fn get_tranche_solution(invest_fulfillment: f64, redeem_fulfillment: f64) -> TrancheSolution {
		TrancheSolution {
			invest_fulfillment: Perquintill::from_float(invest_fulfillment),
			redeem_fulfillment: Perquintill::from_float(redeem_fulfillment),
		}
	}

	fn get_solution(fulfillments: Vec<(f64, f64)>) -> BoundedVec<TrancheSolution, MaxTranches> {
		let mut solutions = Vec::new();

		fulfillments
			.into_iter()
			.for_each(|(invest, redeem)| solutions.push(get_tranche_solution(invest, redeem)));

		BoundedVec::<_, MaxTranches>::truncate_from(solutions)
	}

	fn get_full_solution() -> BoundedVec<TrancheSolution, MaxTranches> {
		let mut solutions = Vec::new();

		solutions.push(get_tranche_solution(1.0, 1.0));
		solutions.push(get_tranche_solution(1.0, 1.0));
		solutions.push(get_tranche_solution(1.0, 1.0));
		solutions.push(get_tranche_solution(1.0, 1.0));

		BoundedVec::<_, MaxTranches>::truncate_from(solutions)
	}

	#[test]
	fn healthy_switches_to_unhealthy() {
		let mut state = PoolState::Healthy;
		state.add_unhealthy(UnhealthyState::MinRiskBufferViolated);
		assert!(state == PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]));

		let mut state = PoolState::Healthy;
		state.update(PoolState::Unhealthy(vec![
			UnhealthyState::MinRiskBufferViolated,
		]));
		assert!(state == PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]));
	}

	#[test]
	fn unhealthy_switches_to_healthy() {
		let mut state = PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]);
		state.update(PoolState::Healthy);
		assert!(state == PoolState::Healthy);
	}

	#[test]
	fn update_overwrites() {
		let mut state = PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]);
		state.update(PoolState::Healthy);
		assert!(state == PoolState::Healthy);

		let mut state = PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]);
		state.update(PoolState::Unhealthy(vec![
			UnhealthyState::MaxReserveViolated,
		]));
		assert!(state == PoolState::Unhealthy(vec![UnhealthyState::MaxReserveViolated]));
	}

	#[test]
	fn unhealthy_always_only_contains_a_single_variant() {
		let mut state = PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]);
		state.add_unhealthy(UnhealthyState::MinRiskBufferViolated);
		assert!(state == PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]));

		state.add_unhealthy(UnhealthyState::MaxReserveViolated);
		assert!(
			state
				== PoolState::Unhealthy(vec![
					UnhealthyState::MinRiskBufferViolated,
					UnhealthyState::MaxReserveViolated
				])
		);
	}

	#[test]
	fn add_unhealthy_works() {
		let mut state = PoolState::Healthy;

		state.add_unhealthy(UnhealthyState::MaxReserveViolated);
		assert!(state == PoolState::Unhealthy(vec![UnhealthyState::MaxReserveViolated]));
	}

	#[test]
	fn rm_unhealthy_works() {
		let mut state = PoolState::Healthy;
		state.rm_unhealthy(UnhealthyState::MaxReserveViolated);
		assert!(state == PoolState::Healthy);

		let mut state = PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]);

		state.add_unhealthy(UnhealthyState::MaxReserveViolated);
		assert!(
			state
				== PoolState::Unhealthy(vec![
					UnhealthyState::MinRiskBufferViolated,
					UnhealthyState::MaxReserveViolated
				])
		);

		state.rm_unhealthy(UnhealthyState::MaxReserveViolated);
		assert!(state == PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]));

		state.rm_unhealthy(UnhealthyState::MaxReserveViolated);
		assert!(state == PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated]));

		state.rm_unhealthy(UnhealthyState::MinRiskBufferViolated);
		assert!(state == PoolState::Healthy);
	}

	#[test]
	fn epoch_solution_healthy_works() {
		let solution = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 0,
		});
		assert!(solution.healthy());

		let solution = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(!solution.healthy());
	}

	#[test]
	fn epoch_solution_solution_works() {
		let solution = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 0,
		});
		assert!(solution.solution() == get_full_solution().as_slice());

		let solution = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution.solution() == get_full_solution().as_slice());
	}

	#[test]
	fn epoch_solution_partial_eq_works() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});
		assert!(solution_1 == solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_solution(vec![(0.0, 0.0), (1.0, 0.7), (0.7, 0.7)]),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 4,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 4,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution_1 == solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_solution(vec![(0.0, 0.0), (1.0, 0.7), (0.7, 0.7)]),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution_1 != solution_2);
	}

	#[test]
	fn unhealthy_solution_has_state_works() {
		let unhealthy = UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: get_full_solution(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		};

		assert!(unhealthy.has_state(&UnhealthyState::MaxReserveViolated));
		assert!(!unhealthy.has_state(&UnhealthyState::MinRiskBufferViolated));
	}

	// Here we start with tests that cover the scoring behaviour which is
	// implemented via the `ParitalOrd` implementation of `EpochSolution`,
	// `HealthySolution` and `UnhealthySolution`.
	#[test]
	fn higher_score_is_better() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 4,
		});
		assert!(solution_1 < solution_2);
	}

	#[test]
	fn healthy_always_above_unhealthy() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MinRiskBufferViolated,
				UnhealthyState::MaxReserveViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(1000),
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 2u128, 3u128, 4u128],
			)), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Healthy(HealthySolution {
			solution: Default::default(),
			score: 0,
		});
		assert!(solution_1 < solution_2);
	}

	#[test]
	fn reserve_improvement_better() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(6),
			risk_buffer_improvement_scores: None,
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn no_reserve_violation_better() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: Default::default(),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: None,
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn no_risk_buff_violation_better() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
				UnhealthyState::MinRiskBufferViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 2u128, 3u128, 4u128],
			)), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(1000),
			risk_buffer_improvement_scores: None,
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn reserve_improvement_decides_over_equal_min_risk_buff() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
				UnhealthyState::MinRiskBufferViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 2u128, 3u128, 4u128],
			)), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::<_, MaxTranches>::truncate_from(vec![
				UnhealthyState::MaxReserveViolated,
				UnhealthyState::MinRiskBufferViolated,
			]),
			solution: Default::default(),
			reserve_improvement_score: Some(6),
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 2u128, 3u128, 4u128],
			)), // 4 tranches
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn risk_buff_improvement_better() {
		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::truncate_from(vec![UnhealthyState::MinRiskBufferViolated]),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 2u128, 3u128, 4u128],
			)), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::truncate_from(vec![UnhealthyState::MinRiskBufferViolated]),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(BoundedVec::truncate_from(vec![
				2u128, 0u128, 0u128, 0u128,
			])), // 4 tranches
		});

		assert!(solution_1 < solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::truncate_from(vec![UnhealthyState::MinRiskBufferViolated]),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 2u128, 3u128, 4u128],
			)), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::truncate_from(vec![UnhealthyState::MinRiskBufferViolated]),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(BoundedVec::truncate_from(vec![
				1u128, 2u128, 3u128, 5u128,
			])), // 4 tranches
		});

		assert!(solution_1 < solution_2);

		let solution_1 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::truncate_from(vec![UnhealthyState::MinRiskBufferViolated]),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 2u128, 3u128, 4u128],
			)), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128, MaxTranches>::Unhealthy(UnhealthySolution {
			state: BoundedVec::truncate_from(vec![UnhealthyState::MinRiskBufferViolated]),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(BoundedVec::<_, MaxTranches>::truncate_from(
				vec![1u128, 3u128, 3u128, 5u128],
			)), // 4 tranches
		});

		assert!(solution_1 < solution_2);
	}
}
