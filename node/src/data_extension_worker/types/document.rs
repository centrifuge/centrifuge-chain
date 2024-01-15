use cfg_primitives::AccountId;
use sc_network::PeerId;
use serde::{Deserialize, Serialize};

pub trait Document<'d>: Clone + Send + Sync + Serialize + Deserialize<'d> + 'static {
	type Id: Serialize + Deserialize<'d> + Send;
	type Version: Serialize + Deserialize<'d> + Send;
	type Users;

	fn get_id(&self) -> Self::Id;

	fn get_version(&self) -> Self::Version;

	fn get_users(&self) -> Self::Users;

	fn set_users(&mut self, users: Self::Users);
}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum UserRole {
	Owner,
	Editor,
	Reader,
}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct UserIdentifier {
	pub substrate_acc_id: AccountId,
	// TODO(cdamian): PeerId needs serialize/deserialize.
	// pub p2p_peer_id: PeerId,
}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct User {
	pub identifier: UserIdentifier,
	pub role: UserRole,
}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct DataExtensionWorkerDocument {
	users: Vec<User>,
	data: Vec<u8>,
}

impl<'d> Document<'d> for DataExtensionWorkerDocument {
	type Id = ();
	type Users = ();
	type Version = ();

	fn get_id(&self) -> Self::Id {
		todo!()
	}

	fn get_version(&self) -> Self::Version {
		todo!()
	}

	fn get_users(&self) -> Self::Users {
		todo!()
	}

	fn set_users(&mut self, _users: Self::Users) {
		todo!()
	}
}
