pub trait TraitA {
	fn foo(p1: String, p2: Option<u64>);
	fn bar(p1: u64, p2: bool) -> Result<(), String>;
}

pub trait TraitB {
	fn qux(p1: String) -> bool;
	fn generic_input<A: Into<i32>>(a: A, b: impl Into<u32>) -> usize;
	fn generic_output<A: Into<i32>>() -> A;
	fn reference(a: &i32) -> &i32;
}

#[frame_support::pallet]
pub mod pallet_mock_ab {
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {}

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
		pub fn mock_foo(f: impl Fn(String, Option<u64>) + 'static) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_bar(f: impl Fn(u64, bool) -> Result<(), String> + 'static) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_qux(f: impl Fn(String) -> bool + 'static) {
			register_call!(f);
		}

		pub fn mock_generic_input<A: Into<i32>, B: Into<u32>>(f: impl Fn(A, B) -> usize + 'static) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_generic_output<A: Into<i32>>(f: impl Fn() -> A + 'static) {
			register_call!(move |()| f());
		}

		pub fn mock_reference(f: impl Fn(&i32) -> &i32 + 'static) {
			register_call!(f);
		}
	}

	impl<T: Config> super::TraitA for Pallet<T> {
		fn foo(a: String, b: Option<u64>) {
			execute_call!((a, b))
		}

		fn bar(a: u64, b: bool) -> Result<(), String> {
			execute_call!((a, b))
		}
	}

	impl<T: Config> super::TraitB for Pallet<T> {
		fn qux(a: String) -> bool {
			execute_call!(a)
		}

		fn generic_input<A: Into<i32>>(a: A, b: impl Into<u32>) -> usize {
			execute_call!((a, b))
		}

		fn generic_output<A: Into<i32>>() -> A {
			execute_call!(())
		}

		fn reference(a: &i32) -> &i32 {
			execute_call!(a)
		}
	}
}

#[frame_support::pallet]
pub mod my_pallet {
	use super::{TraitA, TraitB};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type ActionAB: TraitA + TraitB;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	impl<T: Config> Pallet<T> {
		pub fn my_call(name: &str, value: u64) -> Result<(), String> {
			T::ActionAB::foo(name.into(), Some(value));
			let answer = T::ActionAB::qux(name.into());
			T::ActionAB::bar(value, answer)
		}
	}
}

mod mock {
	use frame_support::traits::{ConstU16, ConstU32, ConstU64};
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
	};

	use super::{my_pallet, pallet_mock_ab};

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
	type Block = frame_system::mocking::MockBlock<Runtime>;

	frame_support::construct_runtime!(
		pub enum Runtime where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system,
			MockAB: pallet_mock_ab,
			MyPallet: my_pallet,
		}
	);

	impl frame_system::Config for Runtime {
		type AccountData = ();
		type AccountId = u64;
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockHashCount = ConstU64<250>;
		type BlockLength = ();
		type BlockNumber = u64;
		type BlockWeights = ();
		type DbWeight = ();
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Header = Header;
		type Index = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type MaxConsumers = ConstU32<16>;
		type OnKilledAccount = ();
		type OnNewAccount = ();
		type OnSetCode = ();
		type PalletInfo = PalletInfo;
		type RuntimeCall = RuntimeCall;
		type RuntimeEvent = RuntimeEvent;
		type RuntimeOrigin = RuntimeOrigin;
		type SS58Prefix = ConstU16<42>;
		type SystemWeightInfo = ();
		type Version = ();
	}

	impl pallet_mock_ab::Config for Runtime {}

	impl my_pallet::Config for Runtime {
		type ActionAB = pallet_mock_ab::Pallet<Runtime>;
	}

	pub fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}
}

mod test {
	use frame_support::assert_ok;

	use super::{mock::*, TraitB};

	#[test]
	fn correct() {
		new_test_ext().execute_with(|| {
			MockAB::mock_foo(|p1, _| assert_eq!("hello", &p1));
			MockAB::mock_qux(|p1| &p1 == "hello");
			MockAB::mock_bar(|_, p2| match p2 {
				true => Ok(()),
				false => Err("err".into()),
			});

			assert_ok!(MyPallet::my_call("hello".into(), 42));
		});
	}

	#[test]
	#[should_panic]
	fn wrong() {
		new_test_ext().execute_with(|| {
			MockAB::mock_foo(|p1, _| assert_eq!("hello", &p1));

			assert_ok!(MyPallet::my_call("bye".into(), 42));
		});
	}

	#[test]
	#[should_panic]
	fn mock_not_configured() {
		new_test_ext().execute_with(|| {
			assert_ok!(MyPallet::my_call("hello".into(), 42));
		});
	}

	#[test]
	#[should_panic]
	fn previous_mock_data_is_destroyed() {
		correct();
		// The storage is dropped at this time. Mocks no longer found from here.
		mock_not_configured();
	}

	#[test]
	fn generic_input() {
		new_test_ext().execute_with(|| {
			MockAB::mock_generic_input(|p1: i8, p2: u8| {
				assert_eq!(p1, 1);
				assert_eq!(p2, 2);
				8
			});
			MockAB::mock_generic_input(|p1: i16, p2: u16| {
				assert_eq!(p1, 3);
				assert_eq!(p2, 4);
				16
			});

			assert_eq!(MockAB::generic_input(1i8, 2u8), 8);
			assert_eq!(MockAB::generic_input(3i16, 4u16), 16);
		});
	}

	#[test]
	#[should_panic]
	fn generic_input_not_found() {
		new_test_ext().execute_with(|| {
			MockAB::mock_generic_input(|p1: i8, p2: u8| {
				assert_eq!(p1, 3);
				assert_eq!(p2, 4);
				8
			});

			MockAB::generic_input(3i16, 4u16);
		});
	}

	#[test]
	fn generic_output() {
		new_test_ext().execute_with(|| {
			MockAB::mock_generic_output(|| 8i8);
			MockAB::mock_generic_output(|| 16i16);

			assert_eq!(MockAB::generic_output::<i8>(), 8);
			assert_eq!(MockAB::generic_output::<i16>(), 16);
		});
	}

	#[test]
	fn reference() {
		new_test_ext().execute_with(|| {
			MockAB::mock_reference(|a| a);

			assert_eq!(MockAB::reference(&42), &42);
		});
	}
}
