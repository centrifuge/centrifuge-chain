//! mod to build fuzzy tests based on commands that needs to be processed
//! without modify the invariants

use std::collections::BTreeSet;

use super::*;

type Account = u64;
type Balance = u64;
type Group = u32;

#[derive(Clone)]
enum Command {
	AttachCurrency((DomainId, CurrencyId), Group),
	Stake((DomainId, CurrencyId), Account, Balance),
	Unstake((DomainId, CurrencyId), Account, Balance),
	Claim((DomainId, CurrencyId), Account),
	Distribute(Vec<Group>, Balance),
}

struct TestState<Rewards> {
	total_distributed: Balance,
	accounts_used: BTreeSet<Account>,
	_reward_system_t: std::marker::PhantomData<Rewards>,
}

impl<Rewards> Default for TestState<Rewards> {
	fn default() -> Self {
		Self {
			total_distributed: 0,
			accounts_used: BTreeSet::default(),
			_reward_system_t: Default::default(),
		}
	}
}

impl<Rewards> TestState<Rewards>
where
	Rewards: DistributedRewards<GroupId = u32, Balance = Balance>
		+ AccountRewards<Account, Balance = Balance, CurrencyId = (DomainId, CurrencyId)>,
{
	fn apply_command(&mut self, command: Command) -> DispatchResult {
		match command {
			Command::AttachCurrency(dom_curr, group) => {
				Rewards1::attach_currency(dom_curr, group)?;
			}
			Command::Stake(dom_curr, account, amount) => {
				Rewards::deposit_stake(dom_curr, &account, amount)?;
				self.accounts_used.insert(account);
			}
			Command::Unstake(dom_curr, account, amount) => {
				Rewards::withdraw_stake(dom_curr, &account, amount)?;
				self.accounts_used.insert(account);
			}
			Command::Claim(dom_curr, account) => {
				Rewards::claim_reward(dom_curr, &account)?;
				self.accounts_used.insert(account);
			}
			Command::Distribute(groups, reward) => {
				self.total_distributed += Rewards::distribute_reward(reward, groups)?
					.iter()
					.filter_map(|e| e.ok())
					.fold(0, |acc, group_reward| acc + group_reward);
			}
		};

		Ok(())
	}

	/// Checks the system invariant. Valid for `base` and `gap` mechanisms
	fn validate(&mut self) {
		let total_claimed = self.accounts_used.iter().fold(0, |acc, account| {
			acc + free_balance(CurrencyId::Reward, account)
		});

		assert_eq!(self.total_distributed, total_claimed + rewards_account());
	}
}

fn evaluate_sample<Rewards>(commands: impl IntoIterator<Item = Command>)
where
	Rewards: DistributedRewards<GroupId = u32, Balance = Balance>
		+ AccountRewards<Account, Balance = Balance, CurrencyId = (DomainId, CurrencyId)>,
{
	new_test_ext().execute_with(|| {
		let mut state = TestState::<Rewards>::default();

		for command in commands {
			// We do not care if we fail applying the command.
			// We only care if the invariant is preserved even if we fail doing it.
			state.apply_command(command.clone()).ok();
		}

		state.validate();
	});
}

/// A sample that emulates a fuzzer that only generates and tests one hardcoded command combination
/// from the following matrix:
///
/// | Action     | Participants | Calls  | Total  |
/// |-           |-             |-       |-       |
/// | stake      | A, B         | 2      | 4      |
/// | unstake    | A, B         | 2      | 4      |
/// | claim      | A, B         | 2      | 4      |
/// | distribute |  -           | 2      | 2      |
///
/// It uses 1 group, 1 domain and 1 currency.
/// It uses the `base` mechanism.
#[test]
fn silly_sample_for_fuzzer() {
	const DOM_CURR: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::A);
	const AMOUNT_A1: u64 = 100;
	const AMOUNT_B1: u64 = 200;
	const AMOUNT_A2: u64 = 300;
	const AMOUNT_B2: u64 = 400;

	let commands = [
		Command::AttachCurrency(DOM_CURR, GROUP_1),
		Command::Stake(DOM_CURR, USER_A, AMOUNT_A1),
		Command::Stake(DOM_CURR, USER_A, AMOUNT_A2),
		Command::Stake(DOM_CURR, USER_B, AMOUNT_B1),
		Command::Stake(DOM_CURR, USER_B, AMOUNT_B2),
		Command::Distribute(vec![GROUP_1], REWARD),
		Command::Claim(DOM_CURR, USER_A),
		Command::Claim(DOM_CURR, USER_B),
		Command::Unstake(DOM_CURR, USER_A, AMOUNT_A1),
		Command::Unstake(DOM_CURR, USER_A, AMOUNT_A2),
		Command::Unstake(DOM_CURR, USER_B, AMOUNT_B1),
		Command::Unstake(DOM_CURR, USER_B, AMOUNT_B2),
		Command::Claim(DOM_CURR, USER_A),
		Command::Claim(DOM_CURR, USER_B),
		Command::Distribute(vec![GROUP_1], REWARD),
	];

	evaluate_sample::<Rewards1>(commands);
}
