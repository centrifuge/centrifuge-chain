use serde::{Deserialize, Serialize};

pub trait Document<'d>: Clone + Send + Sync + Serialize + Deserialize<'d> + 'static {
	type Id: Serialize + Deserialize<'d> + Send;
	type PoolId: Serialize + Deserialize<'d> + Send;
	type LoanId: Serialize + Deserialize<'d> + Send;
	type Version: Serialize + Deserialize<'d> + Send;
	type Fields;
	type Metadata;
	type Users;

	fn get_id(&self) -> Self::Id;

	fn get_pool_id(&self) -> Self::PoolId;

	fn get_loan_id(&self) -> Self::LoanId;

	fn get_version(&self) -> Self::Version;

	fn get_fields(&self) -> Self::Fields;

	fn set_fields(&mut self, fields: Self::Fields);

	fn get_metadata(&self) -> Self::Metadata;

	fn set_metadata(&mut self, metadata: Self::Metadata);

	fn get_users(&self) -> Self::Users;

	fn set_users(&mut self, users: Self::Users);
}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct DataExtensionWorkerDocument {}

impl<'d> Document<'d> for DataExtensionWorkerDocument {
	type Fields = ();
	type Id = ();
	type LoanId = ();
	type Metadata = ();
	type PoolId = ();
	type Users = ();
	type Version = ();

	fn get_id(&self) -> Self::Id {
		todo!()
	}

	fn get_pool_id(&self) -> Self::PoolId {
		todo!()
	}

	fn get_loan_id(&self) -> Self::LoanId {
		todo!()
	}

	fn get_version(&self) -> Self::Version {
		todo!()
	}

	fn get_fields(&self) -> Self::Fields {
		todo!()
	}

	fn set_fields(&mut self, _fields: Self::Fields) {
		todo!()
	}

	fn get_metadata(&self) -> Self::Metadata {
		todo!()
	}

	fn set_metadata(&mut self, _metadata: Self::Metadata) {
		todo!()
	}

	fn get_users(&self) -> Self::Users {
		todo!()
	}

	fn set_users(&mut self, _users: Self::Users) {
		todo!()
	}
}
