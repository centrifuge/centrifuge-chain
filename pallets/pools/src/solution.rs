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
	/// Updates a PoolState to update.
	///
	/// NOTE:
	/// * This will switch a PoolState::Healthy -> PoolState::Unhealthy(_) and vice versa
	/// * If an already unhealthy state is updated, the new Vec<UnhealthyState> inside the
	///   enum will be **overwritten** with the newly passed unhealthy states.
	///   -> Use `add_unhealthy` or `rm_unhealthy` if the other states should be kept.
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
				let mut states = Vec::new();
				states.push(add);
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

				if states.len() == 0 {
					*self = PoolState::Healthy;
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
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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

impl<Balance> PartialOrd for EpochSolution<Balance>
where
	Balance: PartialOrd,
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

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct HealthySolution<Balance> {
	pub solution: Vec<TrancheSolution>,
	pub score: Balance,
}

impl<Balance> PartialOrd for HealthySolution<Balance>
where
	Balance: PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.score.partial_cmp(&other.score)
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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

		// If there are no differences in risk buffer scores or there is no risk buffer violation
		// we look at the reserve improvement score.
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

// The solution struct for a specific tranche
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, Copy)]
pub struct TrancheSolution {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
}

#[cfg(test)]
mod test {
	use super::*;

	fn get_tranche_solution(invest_fulfillment: f64, redeem_fulfillment: f64) -> TrancheSolution {
		TrancheSolution {
			invest_fulfillment: Perquintill::from_float(invest_fulfillment),
			redeem_fulfillment: Perquintill::from_float(redeem_fulfillment),
		}
	}

	fn get_solution(fulfillments: Vec<(f64, f64)>) -> Vec<TrancheSolution> {
		let mut solutions = Vec::new();

		fulfillments
			.into_iter()
			.for_each(|(invest, redeem)| solutions.push(get_tranche_solution(invest, redeem)));

		solutions
	}

	fn get_full_solution() -> Vec<TrancheSolution> {
		let mut solutions = Vec::new();

		solutions.push(get_tranche_solution(1.0, 1.0));
		solutions.push(get_tranche_solution(1.0, 1.0));
		solutions.push(get_tranche_solution(1.0, 1.0));
		solutions.push(get_tranche_solution(1.0, 1.0));

		solutions
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
		let solution = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 0,
		});
		assert!(solution.healthy());

		let solution = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(!solution.healthy());
	}

	#[test]
	fn epoch_solution_solution_works() {
		let solution = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 0,
		});
		assert!(solution.solution() == get_full_solution());

		let solution = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution.solution() == get_full_solution());
	}

	#[test]
	fn epoch_solution_partial_eq_works() {
		let solution_1 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});
		assert!(solution_1 == solution_2);

		let solution_1 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_solution(vec![(0.0, 0.0), (1.0, 0.7), (0.7, 0.7)]),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 4,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 4,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution_1 == solution_2);

		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(2),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution_1 != solution_2);

		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});
		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_solution(vec![(0.0, 0.0), (1.0, 0.7), (0.7, 0.7)]),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});
		assert!(solution_1 != solution_2);
	}

	#[test]
	fn unhealthy_solution_has_state_works() {
		let unhealthy = UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: get_full_solution(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		};

		assert!(unhealthy.has_state(&UnhealthyState::MaxReserveViolated));
		assert!(!unhealthy.has_state(&UnhealthyState::MinRiskBufferViolated));
	}

	// Here we start with tests that cover the scoring behaviour which is implemented
	// via the `ParitalOrd` implementation of `EpochSolution`, `HealthySolution` and `UnhealthySolution`.
	#[test]
	fn higher_score_is_better() {
		let solution_1 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 3,
		});

		let solution_2 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: get_full_solution(),
			score: 4,
		});
		assert!(solution_1 < solution_2);
	}

	#[test]
	fn healthy_always_above_unhealthy() {
		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![
				UnhealthyState::MinRiskBufferViolated,
				UnhealthyState::MaxReserveViolated,
			],
			solution: Default::default(),
			reserve_improvement_score: Some(1000),
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 4u128]), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128>::Healthy(HealthySolution {
			solution: Default::default(),
			score: 0,
		});
		assert!(solution_1 < solution_2);
	}

	#[test]
	fn reserve_improvement_better() {
		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});

		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: Default::default(),
			reserve_improvement_score: Some(6),
			risk_buffer_improvement_scores: None,
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn no_reserve_violation_better() {
		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: None,
		});

		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: Default::default(),
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: None,
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn no_risk_buff_violation_better() {
		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![
				UnhealthyState::MaxReserveViolated,
				UnhealthyState::MinRiskBufferViolated,
			],
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 4u128]), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MaxReserveViolated],
			solution: Default::default(),
			reserve_improvement_score: Some(1000),
			risk_buffer_improvement_scores: None,
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn reserve_improvement_decides_over_equal_min_risk_buff() {
		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![
				UnhealthyState::MaxReserveViolated,
				UnhealthyState::MinRiskBufferViolated,
			],
			solution: Default::default(),
			reserve_improvement_score: Some(5),
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 4u128]), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![
				UnhealthyState::MaxReserveViolated,
				UnhealthyState::MinRiskBufferViolated,
			],
			solution: Default::default(),
			reserve_improvement_score: Some(6),
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 4u128]), // 4 tranches
		});

		assert!(solution_1 < solution_2);
	}

	#[test]
	fn risk_buff_improvement_better() {
		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MinRiskBufferViolated],
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 4u128]), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MinRiskBufferViolated],
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(vec![2u128, 0u128, 0u128, 0u128]), // 4 tranches
		});

		assert!(solution_1 < solution_2);

		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MinRiskBufferViolated],
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 4u128]), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MinRiskBufferViolated],
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 5u128]), // 4 tranches
		});

		assert!(solution_1 < solution_2);

		let solution_1 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MinRiskBufferViolated],
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(vec![1u128, 2u128, 3u128, 4u128]), // 4 tranches
		});

		let solution_2 = EpochSolution::<u128>::Unhealthy(UnhealthySolution {
			state: vec![UnhealthyState::MinRiskBufferViolated],
			solution: Default::default(),
			reserve_improvement_score: None,
			risk_buffer_improvement_scores: Some(vec![1u128, 3u128, 3u128, 5u128]), // 4 tranches
		});

		assert!(solution_1 < solution_2);
	}
}
