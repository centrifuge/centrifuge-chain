use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	pallet_prelude::*, storage::bounded_vec::BoundedVec, storage_alias, traits::OnRuntimeUpgrade,
	weights::Weight, Blake2_128Concat, RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

use crate::{
	write_off::{WriteOffRule, WriteOffTrigger},
	*,
};

mod v0 {
	use super::*;

	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub struct WriteOffState<Rate> {
		pub overdue_days: u32,
		pub percentage: Rate,
		pub penalty: Rate,
	}

	pub type WriteOffStates<T> =
		BoundedVec<v0::WriteOffState<<T as Config>::Rate>, <T as Config>::MaxWriteOffPolicySize>;

	#[storage_alias]
	pub(crate) type WriteOffPolicy<T: Config> =
		StorageMap<Pallet<T>, Blake2_128Concat, PoolIdOf<T>, WriteOffStates<T>, ValueQuery>;
}

/// This updates the policy to the newer version.
pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for Migration<T> {
	fn on_runtime_upgrade() -> Weight {
		if Pallet::<T>::on_chain_storage_version() > StorageVersion::new(0) {
			log::warn!("Migration was already done. This migration can be removed");
			return Weight::zero();
		}

		let mut count = 0;
		WriteOffPolicy::<T>::translate_values(|policy: v0::WriteOffStates<T>| {
			count += 1;
			Some(
				policy
					.into_iter()
					.map(|old| {
						WriteOffRule::new(
							[WriteOffTrigger::PrincipalOverdueDays(old.overdue_days)],
							old.percentage,
							old.penalty,
						)
					})
					.collect::<Vec<_>>()
					.try_into()
					.expect("Size of the new vec can not be longer than previous one, qed"),
			)
		});

		Pallet::<T>::current_storage_version().put::<Pallet<T>>();

		log::info!("Successful migration: v0 -> v1. Items: {count}");

		T::DbWeight::get().reads_writes(count, count)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		Ok(v0::WriteOffPolicy::<T>::iter_values()
			.collect::<Vec<_>>()
			.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
		let old_policy = Vec::<v0::WriteOffStates<T>>::decode(&mut state.as_ref())
			.map_err(|_| "Error decoding pre-upgrade state")?;

		let new_police = WriteOffPolicy::<T>::iter_values();

		old_policy
			.into_iter()
			.zip(new_police)
			.all(|(old_vector, new_vector)| {
				let mut policy = old_vector.iter().zip(new_vector.iter());
				policy.all(|(old, new)| {
					*new == WriteOffRule::new(
						[WriteOffTrigger::PrincipalOverdueDays(old.overdue_days)],
						old.percentage,
						old.penalty,
					)
				})
			})
			.then_some(())
			.ok_or("Error: policies differ")
	}
}

#[cfg(all(test, feature = "try-runtime"))]
mod tests {
	use super::*;
	use crate::mock::*;

	#[test]
	fn migrate() {
		new_test_ext().execute_with(|| {
			v0::WriteOffPolicy::<Runtime>::insert(
				POOL_A,
				BoundedVec::try_from(vec![
					v0::WriteOffState {
						overdue_days: 12,
						percentage: Rate::from_float(0.3),
						penalty: Rate::from_float(0.2),
					},
					v0::WriteOffState {
						overdue_days: 23,
						percentage: Rate::from_float(0.4),
						penalty: Rate::from_float(0.1),
					},
				])
				.unwrap(),
			);

			let pre_state = Migration::<Runtime>::pre_upgrade().unwrap();
			Migration::<Runtime>::on_runtime_upgrade();
			Migration::<Runtime>::post_upgrade(pre_state).unwrap();

			let new_policy = WriteOffPolicy::<Runtime>::get(POOL_A);
			assert_eq!(new_policy.len(), 2);
		});
	}
}
