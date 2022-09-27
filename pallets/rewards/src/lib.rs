#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

//#[cfg(feature = "runtime-benchmarks")]
//mod benchmarking;

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, ReservableCurrency},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;
use num_traits::{NumAssignOps, NumOps, Signed};
use sp_runtime::{
	traits::{AccountIdConversion, BlockNumberProvider, Saturating, Zero},
	FixedPointNumber, FixedPointOperand, TokenError,
};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochDetails<BlockNumber, Balance> {
	epoch: u32,
	ends_on: BlockNumber,
	total_reward: Balance,
}

/// Type used to initialize the first epoch with the correct block number
pub struct FirstEpochDetails<P>(std::marker::PhantomData<P>);
impl<P, N, B: Zero> Get<EpochDetails<N, B>> for FirstEpochDetails<P>
where
	P: BlockNumberProvider<BlockNumber = N>,
{
	fn get() -> EpochDetails<N, B> {
		EpochDetails {
			epoch: 0,
			ends_on: P::current_block_number(),
			total_reward: Zero::zero(),
		}
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct GroupDetails<Balance, Rate> {
	total_staked: Balance,
	deferred_total_staked: Balance,
	reward_per_token: Rate,
}

impl<Balance, Rate> GroupDetails<Balance, Rate>
where
	Balance: NumAssignOps + Copy + Zero + FixedPointOperand,
	Rate: FixedPointNumber<Inner = Balance>,
{
	fn add_amount(&mut self, amount: Balance) {
		self.deferred_total_staked += amount;
	}

	fn sub_amount(&mut self, amount: Balance) {
		deferred_sub(
			&mut self.total_staked,
			&mut self.deferred_total_staked,
			amount,
		);
	}

	fn add_reward(&mut self, reward: Balance) -> bool {
		let should_reward = self.total_staked > Zero::zero();
		if should_reward {
			let rate_increment = Rate::saturating_from_rational(reward, self.total_staked);
			self.reward_per_token = self.reward_per_token + rate_increment;
		}
		self.total_staked = self.deferred_total_staked;
		self.deferred_total_staked = Zero::zero();
		should_reward
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct StakedDetails<Balance, SignedBalance> {
	amount: Balance,
	reward_tally: SignedBalance,
	deferred_amount: Balance,
	deferred_reward_tally: SignedBalance,
	undeferred_epoch: u32,
}

impl<Balance, SignedBalance> StakedDetails<Balance, SignedBalance>
where
	Balance: NumAssignOps + Copy + Zero + FixedPointOperand,
	SignedBalance:
		NumAssignOps + NumOps + Copy + Zero + From<Balance> + TryInto<Balance> + Saturating,
{
	fn try_undeferred(&mut self, current_epoch: u32) {
		if self.undeferred_epoch < current_epoch {
			self.amount += self.deferred_amount;
			self.reward_tally += self.deferred_reward_tally;
			self.deferred_amount = Zero::zero();
			self.deferred_reward_tally = Zero::zero();
			self.undeferred_epoch = current_epoch;
		}
	}

	fn add_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
		current_epoch: u32,
	) {
		self.try_undeferred(current_epoch);
		self.deferred_amount += amount;
		self.deferred_reward_tally += reward_per_token.saturating_mul_int(amount).into();
	}

	fn sub_amount<Rate: FixedPointNumber>(&mut self, amount: Balance, reward_per_token: Rate) {
		deferred_sub(&mut self.amount, &mut self.deferred_amount, amount);

		deferred_sub(
			&mut self.reward_tally,
			&mut self.deferred_reward_tally,
			reward_per_token.saturating_mul_int(amount).into(),
		);
	}

	fn reward<Rate: FixedPointNumber>(
		&mut self,
		reward_per_token: Rate,
		current_epoch: u32,
	) -> Balance {
		self.try_undeferred(current_epoch);

		let gross_reward: SignedBalance = reward_per_token.saturating_mul_int(self.amount).into();
		let reward_tally = self.reward_tally;

		self.reward_tally = gross_reward;

		// Logically this should never be less than 0.
		(gross_reward - reward_tally)
			.try_into()
			.unwrap_or(Zero::zero())
	}
}

/// Substract `amount` from `deferred_value`,
/// if `amount` is greather than `deferred_value`, substrat the reminder from `value`.
fn deferred_sub<S: Saturating + Copy>(value: &mut S, deferred_value: &mut S, amount: S) {
	*value = value.saturating_sub(amount.saturating_sub(*deferred_value));
	*deferred_value = deferred_value.saturating_sub(amount);
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type BlockPerEpoch: Get<Self::BlockNumber>;

		type Currency: ReservableCurrency<Self::AccountId>;

		type SignedBalance: From<BalanceOf<Self>>
			+ TryInto<BalanceOf<Self>>
			+ codec::FullCodec
			+ Copy
			+ Default
			+ scale_info::TypeInfo
			+ MaxEncodedLen
			+ NumOps
			+ NumAssignOps
			+ Saturating
			+ Signed
			+ Zero;

		type Rate: FixedPointNumber<Inner = BalanceOf<Self>>
			+ TypeInfo
			+ MaxEncodedLen
			+ Saturating
			+ Encode
			+ Decode;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// --------------------------
	//          Storage
	// --------------------------

	#[pallet::storage]
	pub type ActiveEpoch<T: Config> = StorageValue<
		_,
		EpochDetails<T::BlockNumber, BalanceOf<T>>,
		ValueQuery,
		FirstEpochDetails<frame_system::Pallet<T>>,
	>;

	#[pallet::storage]
	pub type NextTotalReward<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	#[pallet::storage]
	pub type Group<T: Config> = StorageValue<_, GroupDetails<BalanceOf<T>, T::Rate>, ValueQuery>;

	#[pallet::storage]
	pub type Staked<T: Config> = StorageMap<
		_,
		Blake2_256,
		T::AccountId,
		StakedDetails<BalanceOf<T>, T::SignedBalance>,
		ValueQuery,
	>;

	// --------------------------

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)] // TODO
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			let active_epoch = ActiveEpoch::<T>::get();

			if active_epoch.ends_on != current_block {
				return T::DbWeight::get().reads(1);
			}

			Group::<T>::mutate(|group| {
				if group.add_reward(active_epoch.total_reward) {
					T::Currency::deposit_creating(
						&T::PalletId::get().into_account_truncating(),
						active_epoch.total_reward,
					);
				}
			});

			ActiveEpoch::<T>::put(EpochDetails {
				epoch: active_epoch.epoch + 1,
				ends_on: current_block + T::BlockPerEpoch::get(),
				total_reward: NextTotalReward::<T>::get(),
			});

			T::DbWeight::get().reads_writes(2, 2) // + deposit_creating weight // TODO
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		#[pallet::weight(10_000)] //TODO
		#[transactional]
		pub fn stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			T::Currency::reserve(&who, amount)?;

			Group::<T>::mutate(|group| {
				Staked::<T>::mutate(&who, |staked| {
					staked.add_amount(
						amount,
						group.reward_per_token,
						ActiveEpoch::<T>::get().epoch,
					);
				});

				group.add_amount(amount);
			});

			Ok(())
		}

		#[pallet::weight(10_000)] //TODO
		#[transactional]
		pub fn unstake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if T::Currency::reserved_balance(&who) < amount {
				return Err(DispatchError::Token(TokenError::NoFunds));
			}

			Group::<T>::mutate(|group| {
				Staked::<T>::mutate(&who, |staked| {
					staked.sub_amount(amount, group.reward_per_token);
				});

				group.sub_amount(amount);
			});

			T::Currency::unreserve(&who, amount);

			Ok(())
		}

		#[pallet::weight(10_000)] //TODO
		#[transactional]
		pub fn claim(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let group = Group::<T>::get();

			let reward = Staked::<T>::mutate(&who, |staked| {
				staked.reward(group.reward_per_token, ActiveEpoch::<T>::get().epoch)
			});

			T::Currency::transfer(
				&T::PalletId::get().into_account_truncating(),
				&who,
				reward,
				ExistenceRequirement::KeepAlive,
			)
		}
	}
}
