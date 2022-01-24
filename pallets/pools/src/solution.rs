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

use super::*;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum PoolState {
	Healthy,
	Unhealthy(Vec<UnhealthyState>),
}

impl PoolState {
	pub fn update(&mut self, update: PoolState) -> &mut Self {
		match self {
			PoolState::Healthy => match update {
				PoolState::Healthy => self,
				PoolState::Unhealthy(_) => {
					*self = update;
					self
				}
			},
			PoolState::Unhealthy(states) => match update {
				PoolState::Healthy => {
					*self = update;
					self
				}
				PoolState::Unhealthy(updates_states) => {
					updates_states.into_iter().for_each(|unhealthy| {
						if !states.contains(&unhealthy) {
							states.push(unhealthy)
						}
					});
					self
				}
			},
		}
	}

	pub fn update_with_unhealthy(&mut self, update: UnhealthyState) -> &mut Self {
		match self {
			PoolState::Healthy => {
				let mut states = Vec::new();
				states.push(update);
				*self = PoolState::Unhealthy(states);
				self
			}
			PoolState::Unhealthy(states) => {
				if !states.contains(&update) {
					states.push(update);
				}
				self
			}
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum UnhealthyState {
	MaxReserveViolated,
	MinRiskBufferViolated,
}

/// The solutions struct for epoch solution
#[derive(Encode, Decode, Clone, Eq, RuntimeDebug, TypeInfo)]
pub enum EpochSolution<Balance> {
	Healthy(HealthySolution<Balance>),
	Unhealthy(UnhealthySolution<Balance>),
}

impl<Balance> EpochSolution<Balance>
where
	Balance: Copy,
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

impl<Balance> PartialEq for EpochSolution<Balance>
where
	Balance: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		match self {
			EpochSolution::Healthy(s_1) => match other {
				EpochSolution::Healthy(s_2) => s_1.score == s_2.score,
				EpochSolution::Unhealthy(_) => false,
			},
			EpochSolution::Unhealthy(s_1) => match other {
				EpochSolution::Healthy(_) => false,
				EpochSolution::Unhealthy(s_2) => s_1 == s_2,
			},
		}
	}
}

impl<Balance> PartialOrd for EpochSolution<Balance>
where
	Balance: PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		match self {
			EpochSolution::Healthy(s_1) => match other {
				EpochSolution::Healthy(s_2) => {
					let score_1 = &s_1.score;
					let score_2 = &s_2.score;

					Some(if score_1 > score_2 {
						Ordering::Greater
					} else if score_1 < score_2 {
						Ordering::Less
					} else {
						Ordering::Equal
					})
				}
				EpochSolution::Unhealthy(_) => Some(Ordering::Greater),
			},
			EpochSolution::Unhealthy(s_1) => match other {
				EpochSolution::Healthy(_) => Some(Ordering::Less),
				EpochSolution::Unhealthy(s_2) => s_1.partial_cmp(s_2),
			},
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct HealthySolution<Balance> {
	pub solution: Vec<TrancheSolution>,
	pub score: Balance,
}

#[derive(Encode, Decode, Clone, Eq, RuntimeDebug, TypeInfo)]
pub struct UnhealthySolution<Balance> {
	pub state: Vec<UnhealthyState>,
	pub solution: Vec<TrancheSolution>,
	// The risk buffer score per tranche (less junior tranche) for this solution
	pub risk_buffer_improvement_scores: Option<Vec<Balance>>,
	// The reserve buffer score for this solution
	pub reserve_improvement_score: Option<Balance>,
}

impl<Balance> UnhealthySolution<Balance> {
	fn has_state(&self, state: &UnhealthyState) -> bool {
		self.state.contains(state)
	}
}

impl<Balance> PartialOrd for UnhealthySolution<Balance>
where
	Balance: PartialOrd,
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
				if self.risk_buffer_improvement_scores > other.risk_buffer_improvement_scores {
					return Some(Ordering::Greater);
				} else if self.risk_buffer_improvement_scores > other.risk_buffer_improvement_scores
				{
					return Some(Ordering::Less);
				}
			}
			(false, true) => return Some(Ordering::Greater),
			(true, false) => return Some(Ordering::Less),
			(false, false) => (),
		}

		// If there are no differences in risk buffer scores, we look at the reserve improvement score.
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

		Some(Ordering::Equal)
	}
}

impl<Balance> PartialEq for UnhealthySolution<Balance>
where
	Balance: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		self.risk_buffer_improvement_scores
			.iter()
			.zip(&other.risk_buffer_improvement_scores)
			.map(|(s_1_score, s_2_score)| s_1_score == s_2_score)
			.all(|same_score| same_score)
			&& self.reserve_improvement_score == other.reserve_improvement_score
	}
}

// The solution struct for a specific tranche
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, Copy)]
pub struct TrancheSolution {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
}
