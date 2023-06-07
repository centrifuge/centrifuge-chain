use sp_runtime::DispatchError;

/// Trait for get feedback before apply certain changes.
/// It can be used when you need to ask to a third party or external module if
/// applying a change that has some effect into the system is something healthy.
pub trait ChangeGuard {
	/// Associated pool where evaluate the change.
	type PoolId;

	/// Identification of a change.
	type ChangeId;

	/// Kind of change.
	type Change;

	/// Notify a `change` related to a `pool_id`.
	/// The caller to this method ask for feedback for the implementation of
	/// this trait in order be able to semantically proceed successful with that
	/// change. The change intention will be noted by this method and identified
	/// by the returned ChangeId.
	fn note(pool_id: Self::PoolId, change: Self::Change) -> Result<Self::ChangeId, DispatchError>;

	/// Ask for a `change_id` if it's ready to proceed.
	/// An error will be returned if:
	/// - The change not exists.
	/// - The change is not ready to be applied yet. The conditions not
	///   fulfilled.
	/// - The change was already released.
	/// - The change has expired.
	/// If `Ok()`, the caller can proceed.
	fn released(
		pool_id: Self::PoolId,
		change_id: Self::ChangeId,
	) -> Result<Self::Change, DispatchError>;
}
