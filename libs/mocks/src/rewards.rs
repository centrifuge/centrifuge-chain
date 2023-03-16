#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, GroupRewards};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Balance;
		type GroupId;
		type CurrencyId;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		mock_builder::CallId,
	>;

	impl<T: Config> Pallet<T> {
		pub fn mock_is_ready(f: impl Fn(T::GroupId) -> bool + 'static) {
			register_call!(f);
		}

		pub fn mock_reward_group(
			f: impl Fn(T::GroupId, T::Balance) -> Result<T::Balance, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_group_stake(f: impl Fn(T::GroupId) -> T::Balance + 'static) {
			register_call!(f);
		}

		pub fn mock_deposit_stake(
			f: impl Fn(T::CurrencyId, &T::AccountId, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_withdraw_stake(
			f: impl Fn(T::CurrencyId, &T::AccountId, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_compute_reward(
			f: impl Fn(T::CurrencyId, &T::AccountId) -> Result<T::Balance, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_claim_reward(
			f: impl Fn(T::CurrencyId, &T::AccountId) -> Result<T::Balance, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_account_stake(
			f: impl Fn(T::CurrencyId, &T::AccountId) -> T::Balance + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_attach_currency(
			f: impl Fn(T::CurrencyId, T::GroupId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_currency_group(f: impl Fn(T::CurrencyId) -> Option<T::GroupId> + 'static) {
			register_call!(f);
		}
	}

	impl<T: Config> GroupRewards for Pallet<T> {
		type Balance = T::Balance;
		type GroupId = T::GroupId;

		fn is_ready(a: T::GroupId) -> bool {
			execute_call!(a)
		}

		fn reward_group(a: T::GroupId, b: T::Balance) -> Result<T::Balance, DispatchError> {
			execute_call!((a, b))
		}

		fn group_stake(a: T::GroupId) -> T::Balance {
			execute_call!(a)
		}
	}

	impl<T: Config> AccountRewards<T::AccountId> for Pallet<T> {
		type Balance = T::Balance;
		type CurrencyId = T::CurrencyId;

		fn deposit_stake(a: T::CurrencyId, b: &T::AccountId, c: T::Balance) -> DispatchResult {
			let b = unsafe { std::mem::transmute::<_, &'static T::AccountId>(b) };
			execute_call!((a, b, c))
		}

		fn withdraw_stake(a: T::CurrencyId, b: &T::AccountId, c: T::Balance) -> DispatchResult {
			let b = unsafe { std::mem::transmute::<_, &'static T::AccountId>(b) };
			execute_call!((a, b, c))
		}

		fn compute_reward(a: T::CurrencyId, b: &T::AccountId) -> Result<T::Balance, DispatchError> {
			let b = unsafe { std::mem::transmute::<_, &'static T::AccountId>(b) };
			execute_call!((a, b))
		}

		fn claim_reward(a: T::CurrencyId, b: &T::AccountId) -> Result<T::Balance, DispatchError> {
			let b = unsafe { std::mem::transmute::<_, &'static T::AccountId>(b) };
			execute_call!((a, b))
		}

		fn account_stake(a: T::CurrencyId, b: &T::AccountId) -> T::Balance {
			let b = unsafe { std::mem::transmute::<_, &'static T::AccountId>(b) };
			execute_call!((a, b))
		}
	}

	impl<T: Config> CurrencyGroupChange for Pallet<T> {
		type CurrencyId = T::CurrencyId;
		type GroupId = T::GroupId;

		fn attach_currency(a: T::CurrencyId, b: T::GroupId) -> DispatchResult {
			execute_call!((a, b))
		}

		fn currency_group(a: T::CurrencyId) -> Option<T::GroupId> {
			execute_call!(a)
		}
	}
}
