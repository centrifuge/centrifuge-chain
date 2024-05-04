use cfg_primitives::{AccountId, CFG};
use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, DistributedRewards};
use cfg_types::tokens::CurrencyId;
use frame_support::{assert_ok, dispatch::RawOrigin};
use runtime_common::{
	apis::{runtime_decl_for_rewards_api::RewardsApiV1, RewardDomain},
	instances::BlockRewards,
};
use sp_runtime::traits::{Get, Zero};

use crate::{
	generic::{
		config::{Runtime, RuntimeKind},
		env::Env,
		envs::runtime_env::RuntimeEnv,
		utils,
		utils::{
			currency::cfg,
			genesis::{self, Genesis},
		},
	},
	utils::accounts::{default_accounts, Keyring},
};

#[test_runtimes(all)]
fn block_rewards_api<T: Runtime>() {
	const STAKER: Keyring = Keyring::Alice;

	RuntimeEnv::<T>::default().parachain_state_mut(|| {
		let group_id = 1u32;
		let amount = 100 * CFG;

		utils::give_balance::<T>(STAKER.id(), T::ExistentialDeposit::get() + amount);

		assert_ok!(pallet_rewards::Pallet::<T, BlockRewards>::attach_currency(
			CurrencyId::Native,
			group_id,
		));

		assert_ok!(pallet_rewards::Pallet::<T, BlockRewards>::deposit_stake(
			CurrencyId::Native,
			&STAKER.id(),
			amount,
		));

		assert_ok!(
			pallet_rewards::Pallet::<T, BlockRewards>::distribute_reward(200 * CFG, [group_id])
		);

		assert_eq!(
			T::Api::list_currencies(RewardDomain::Block, STAKER.id()),
			vec![CurrencyId::Native]
		);

		assert_eq!(
			T::Api::compute_reward(RewardDomain::Block, CurrencyId::Native, STAKER.id()),
			Some(200 * CFG)
		)
	});
}

#[test_runtimes(all)]
fn collator_list_synchronized<T: Runtime>() {
	RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(
				T::ExistentialDeposit::get() + cfg(100),
			))
			.add(genesis::invulnerables::<T>(vec![Keyring::Admin]))
			.add(genesis::session_keys::<T>())
			.add(genesis::block_rewards::<T>(vec![Keyring::Admin]))
			.storage(),
	)
	.parachain_state_mut(|| {
		let collators_1 = vec![Keyring::Alice.id(), Keyring::Bob.id()];
		let collators_2 = vec![
			Keyring::Charlie.id(),
			Keyring::Dave.id(),
			Keyring::Eve.id(),
			Keyring::Ferdie.id(),
		];

		// altair and centrifuge use collator_allowlist,
		// so we need to add the accounts there for them.
		if T::KIND == RuntimeKind::Altair || T::KIND == RuntimeKind::Centrifuge {
			for account in default_accounts() {
				assert_ok!(pallet_collator_allowlist::Pallet::<T>::add(
					RawOrigin::Root.into(),
					account.id()
				));
			}
		}

		// SESSION 0 -> 1;
		apply_and_check_session::<T>(1, collators_1.clone(), vec![]);

		// SESSION 1 -> 2;
		apply_and_check_session::<T>(3, collators_2.clone(), vec![]);

		// SESSION 2 -> 3;
		apply_and_check_session::<T>(7, vec![], vec![Keyring::Alice.id()]);

		assert!(collators_1.iter().all(|c| has_reward::<T>(c)));
		assert!(collators_2.iter().all(|c| !has_reward::<T>(c)));

		// SESSION 3 -> 4;
		apply_and_check_session::<T>(6, vec![], vec![]);

		assert!(collators_2.iter().all(|c| has_reward::<T>(c)));
	});
}

fn apply_and_check_session<T: Runtime>(
	collator_count: u32,
	joining: Vec<AccountId>,
	leaving: Vec<AccountId>,
) {
	for collator in &joining {
		pallet_collator_selection::Pallet::<T>::register_as_candidate(
			RawOrigin::Signed(collator.clone()).into(),
		)
		.unwrap();
	}

	for collator in &leaving {
		pallet_collator_selection::Pallet::<T>::leave_intent(
			RawOrigin::Signed(collator.clone()).into(),
		)
		.unwrap();
	}

	frame_system::Pallet::<T>::reset_events();

	pallet_session::Pallet::<T>::rotate_session();

	/* TODO: pending to fix. Why try_into() fails getting the reward_event::GroupRewarded?

	// The event exists in this list:
	dbg!(frame_system::Pallet::<T>::events())

	// But not in this list (that is the implementation of find_event()),
	// so try_into returns an Err for it.
	dbg!(frame_system::Pallet::<T>::events()
		.into_iter()
		.rev()
		.find_map(|record| record.event.try_into().ok())
		.flatten());

	// But later, if manually I create the event as follows:
	let e = T::RuntimeEventExt::from(pallet_rewards::Event::<T, BlockRewards>::GroupRewarded {
		group_id: 1,
		amount: 2,
	});

	// And I call try_into(), it works.
	let re: pallet_rewards::Event<T, BlockRewards> = e.try_into().ok().unwrap();
	*/

	/* // Uncomment once fix the above issue
	utils::find_event::<T, _, _>(|e| match e {
		pallet_rewards::Event::<_, BlockRewards>::GroupRewarded { .. } => Some(true),
		_ => None,
	})
	.unwrap();
	*/

	utils::find_event::<T, _, _>(|e| match e {
		pallet_block_rewards::Event::NewSession { .. } => Some(true),
		_ => None,
	})
	.unwrap();

	// Checks post applying new session:

	assert_eq!(
		pallet_block_rewards::Pallet::<T>::active_session_data().collator_count,
		collator_count,
	);

	if !joining.is_empty() || !leaving.is_empty() {
		assert_eq!(
			pallet_block_rewards::Pallet::<T>::next_session_changes().collator_count,
			Some(collator_count + joining.len() as u32 - leaving.len() as u32)
		);
	}

	let next_collators = pallet_block_rewards::Pallet::<T>::next_session_changes().collators;
	assert_eq!(*next_collators.inc, joining);
	assert_eq!(*next_collators.out, leaving);

	assert!(joining.iter().all(|c| !is_staked::<T>(c)));
	assert!(joining.iter().all(|c| is_candidate::<T>(c)));
	assert!(leaving.iter().all(|c| is_staked::<T>(c)));
	assert!(leaving.iter().all(|c| !is_candidate::<T>(c)));
}

fn has_reward<T: Runtime>(collator: &AccountId) -> bool {
	!pallet_rewards::Pallet::<T, BlockRewards>::compute_reward(T::StakeCurrencyId::get(), collator)
		.unwrap()
		.is_zero()
}

fn is_staked<T: Runtime>(collator: &AccountId) -> bool {
	!pallet_rewards::Pallet::<T, BlockRewards>::account_stake(T::StakeCurrencyId::get(), collator)
		.is_zero()
}

fn is_candidate<T: Runtime>(collator: &AccountId) -> bool {
	pallet_collator_selection::Pallet::<T>::candidates()
		.into_iter()
		.find(|c| c.who == *collator)
		.is_some()
}
