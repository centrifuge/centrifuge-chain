use frame_support::traits::GenesisBuild;
use sp_runtime::Storage;

use crate::generic::runtime::Runtime;

#[derive(Default)]
pub struct Genesis {
	storage: Storage,
}

impl Genesis {
	pub fn add<T: Runtime>(mut self, builder: impl GenesisBuild<T>) -> Genesis {
		builder.assimilate_storage(&mut self.storage).unwrap();
		self
	}

	pub fn storage(self) -> Storage {
		self.storage
	}
}
