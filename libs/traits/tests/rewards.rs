use cfg_mocks::pallet_mock_rewards;
use cfg_traits::rewards::DistributedRewards;
use frame_support::assert_ok;
use sp_runtime::DispatchError;

const REWARD_ZERO: u64 = 0;
const REWARD: u64 = 100;

#[derive(Clone)]
pub enum GroupId {
	Empty,
	Err,
	A,
	B,
}

impl pallet_mock_rewards::Config for Runtime {
	type Balance = u64;
	type CurrencyId = ();
	type GroupId = GroupId;
}

cfg_mocks::make_runtime_for_mock!(Runtime, MockRewards, pallet_mock_rewards, new_test_ext);

fn config_mocks() {
	MockRewards::mock_is_ready(|group_id| match group_id {
		GroupId::Empty => false,
		_ => true,
	});
	MockRewards::mock_reward_group(|group_id, reward| match group_id {
		GroupId::Err => Err(DispatchError::Other("issue")),
		_ => Ok(reward),
	});
}

#[test]
fn distribute_zero() {
	new_test_ext().execute_with(|| {
		config_mocks();
		assert_ok!(
			MockRewards::distribute_reward(
				REWARD_ZERO,
				[GroupId::Empty, GroupId::Err, GroupId::A, GroupId::B]
			),
			vec![Ok(0), Err(DispatchError::Other("issue")), Ok(0), Ok(0)]
		);
	});
}

#[test]
fn distribute_to_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(MockRewards::distribute_reward(REWARD, []), vec![]);
	});
}

#[test]
fn distribute_the_same() {
	new_test_ext().execute_with(|| {
		config_mocks();
		assert_ok!(
			MockRewards::distribute_reward(
				REWARD,
				[GroupId::Empty, GroupId::Err, GroupId::A, GroupId::B]
			),
			vec![
				Ok(0),
				Err(DispatchError::Other("issue")),
				Ok(REWARD / 3),
				Ok(REWARD / 3)
			]
		);
	});
}

#[test]
fn distribute_with_weights() {
	new_test_ext().execute_with(|| {
		config_mocks();
		assert_ok!(
			MockRewards::distribute_reward_with_weights(
				REWARD,
				[
					(GroupId::Empty, 10u32),
					(GroupId::Err, 20u32),
					(GroupId::A, 30u32),
					(GroupId::B, 40u32)
				]
			),
			vec![Ok(0), Err(DispatchError::Other("issue")), Ok(33), Ok(44)]
		);
	});
}
