//! mod to build fuzzy tests based on commands that needs to be processed
//! without modify the invariants

use std::collections::BTreeSet;

use super::*;

type Account = u64;
type Balance = u64;
type Group = u32;

#[derive(Clone)]
enum Command {
	AttachCurrency(CurrencyId, Group),
	Stake(CurrencyId, Account, Balance),
	Unstake(CurrencyId, Account, Balance),
	Claim(CurrencyId, Account),
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
		+ AccountRewards<Account, Balance = Balance, CurrencyId = CurrencyId>,
{
	fn apply_command(&mut self, command: Command) -> DispatchResult {
		match command {
			Command::AttachCurrency(currency, group) => {
				Rewards1::attach_currency(currency, group)?;
			}
			Command::Stake(currency, account, amount) => {
				Rewards::deposit_stake(currency, &account, amount)?;
				self.accounts_used.insert(account);
			}
			Command::Unstake(currency, account, amount) => {
				Rewards::withdraw_stake(currency, &account, amount)?;
				self.accounts_used.insert(account);
			}
			Command::Claim(currency, account) => {
				Rewards::claim_reward(currency, &account)?;
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
		+ AccountRewards<Account, Balance = Balance, CurrencyId = CurrencyId>,
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

/// A sample that emulates a fuzzer that only generates and tests one hardcoded
/// command combination from the following matrix:
///
/// | Action     | Participants | Calls  | Total  |
/// |-           |-             |-       |-       |
/// | stake      | A, B         | 2      | 4      |
/// | unstake    | A, B         | 2      | 4      |
/// | claim      | A, B         | 2      | 4      |
/// | distribute |  -           | 2      | 2      |
///
/// It uses 1 group and 1 currency.
/// It uses the `base` mechanism.
#[test]
fn silly_sample_for_fuzzer() {
	const CURR: CurrencyId = CurrencyId::A;
	const AMOUNT_A1: u64 = 100;
	const AMOUNT_B1: u64 = 200;
	const AMOUNT_A2: u64 = 300;
	const AMOUNT_B2: u64 = 400;

	let commands = [
		Command::AttachCurrency(CURR, GROUP_1),
		Command::Stake(CURR, USER_A, AMOUNT_A1),
		Command::Stake(CURR, USER_A, AMOUNT_A2),
		Command::Stake(CURR, USER_B, AMOUNT_B1),
		Command::Stake(CURR, USER_B, AMOUNT_B2),
		Command::Distribute(vec![GROUP_1], REWARD),
		Command::Claim(CURR, USER_A),
		Command::Claim(CURR, USER_B),
		Command::Unstake(CURR, USER_A, AMOUNT_A1),
		Command::Unstake(CURR, USER_A, AMOUNT_A2),
		Command::Unstake(CURR, USER_B, AMOUNT_B1),
		Command::Unstake(CURR, USER_B, AMOUNT_B2),
		Command::Claim(CURR, USER_A),
		Command::Claim(CURR, USER_B),
		Command::Distribute(vec![GROUP_1], REWARD),
	];

	evaluate_sample::<Rewards1>(commands);
}
