use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	pallet_prelude::*, storage::bounded_vec::BoundedVec, storage_alias, traits::OnRuntimeUpgrade,
	weights::Weight, Blake2_128Concat, RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_std::collections::btree_set::BTreeSet;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

use crate::{
	types::{WriteOffRule, WriteOffStatus, WriteOffTrigger},
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
					.map(|old| WriteOffRule {
						triggers: BTreeSet::from_iter([WriteOffTrigger::PrincipalOverdueDays(
							old.overdue_days,
						)])
						.try_into()
						.expect("We have at least 1 element in the enum, qed"),
						status: WriteOffStatus {
							percentage: old.percentage,
							penalty: old.penalty,
						},
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
					let trigger = new
						.triggers
						.contains(&WriteOffTrigger::PrincipalOverdueDays(old.overdue_days));

					trigger
						&& old.percentage == new.status.percentage
						&& old.penalty == new.status.penalty
				})
			})
			.then_some(())
			.ok_or("Error: policies differ")
	}
}
