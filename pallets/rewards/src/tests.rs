mod common;
mod currency_movement;

use cfg_traits::rewards::DistributedRewards;
use frame_support::{assert_noop, assert_ok, traits::fungibles::Inspect};

use super::{mock::*, *};

const GROUP_A: u32 = 1;
const GROUP_B: u32 = 2;
const GROUP_C: u32 = 3;

const DOM_1_CURRENCY_A: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::A);
const DOM_1_CURRENCY_B: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::B);
const DOM_1_CURRENCY_C: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::C);
const DOM_1_CURRENCY_M: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::M);

const STAKE_A: u64 = 100;
const STAKE_B: u64 = 200;
const STAKE_C: u64 = 300;
const STAKE_M: u64 = 400;

const REWARD: u64 = 120;

#[derive(Clone, Copy, PartialEq)]
enum MechanismKind {
	Base,
	Gap,
}

fn free_balance(currency_id: CurrencyId, account_id: &u64) -> u64 {
	Tokens::reducible_balance(currency_id, account_id, true)
}

fn rewards_account() -> u64 {
	Tokens::balance(
		CurrencyId::Reward,
		&RewardsPalletId::get().into_account_truncating(),
	)
}

fn choose_balance(kind: MechanismKind, base: u64, gap: u64) -> u64 {
	match kind {
		MechanismKind::Base => base,
		MechanismKind::Gap => gap,
	}
}

mod mechanism {
	use super::*;

	mod base {
		use super::*;

		common_tests!(Rewards1, Instance1, MechanismKind::Base);
		currency_movement_tests!(Rewards1, Instance1, MechanismKind::Base);
	}

	mod gap {
		use super::*;

		common_tests!(Rewards3, Instance3, MechanismKind::Gap);
		currency_movement_tests!(Rewards3, Instance3, MechanismKind::Gap);
	}
}
