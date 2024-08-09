use cfg_traits::liquidity_pools::Router;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use mock_builder::{execute_call, register_call};
use sp_std::default::Default;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_init(f: impl Fn() -> DispatchResult + 'static) {
			register_call!(move |()| f());
		}

		pub fn mock_send(
			f: impl Fn(T::AccountId, Vec<u8>) -> DispatchResultWithPostInfo + 'static,
		) {
			register_call!(move |(sender, message)| f(sender, message));
		}

		pub fn mock_hash(f: impl Fn() -> T::Hash + 'static) {
			register_call!(move |()| f());
		}
	}

	impl<T: Config> MockedRouter for Pallet<T> {
		type Hash = T::Hash;
		type Sender = T::AccountId;

		fn init() -> DispatchResult {
			execute_call!(())
		}

		fn send(sender: Self::Sender, message: Vec<u8>) -> DispatchResultWithPostInfo {
			execute_call!((sender, message))
		}

		fn hash() -> Self::Hash {
			execute_call!(())
		}
	}
}

/// This wraps the mocking functionality of the pallet that we build here and is
/// necessary since this will kept in storage, therefore it has to implement the
/// below traits that make that possible.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct RouterMock<T> {
	_marker: PhantomData<T>,
}

impl<T: pallet::Config> Default for RouterMock<T> {
	fn default() -> Self {
		RouterMock::<T> {
			_marker: Default::default(),
		}
	}
}

impl<T: pallet::Config> RouterMock<T> {
	pub fn mock_init(&self, f: impl Fn() -> DispatchResult + 'static) {
		pallet::Pallet::<T>::mock_init(f)
	}

	pub fn mock_send(
		&self,
		f: impl Fn(T::AccountId, Vec<u8>) -> DispatchResultWithPostInfo + 'static,
	) {
		pallet::Pallet::<T>::mock_send(f)
	}

	pub fn mock_hash(&self, f: impl Fn() -> <RouterMock<T> as Router>::Hash + 'static) {
		pallet::Pallet::<T>::mock_hash(f)
	}
}

/// Here we implement the actual Router trait for the `RouterMock` which in turn
/// calls the `MockedRouter` trait implementation.
impl<T: pallet::Config> Router for RouterMock<T> {
	type Hash = T::Hash;
	type Sender = T::AccountId;

	fn init(&self) -> DispatchResult {
		pallet::Pallet::<T>::init()
	}

	fn send(&self, sender: Self::Sender, message: Vec<u8>) -> DispatchResultWithPostInfo {
		pallet::Pallet::<T>::send(sender, message)
	}

	fn hash(&self) -> Self::Hash {
		pallet::Pallet::<T>::hash()
	}
}

/// A mocked Router trait that emulates the actual Router trait but without
/// the inclusion of &self in the function parameters. This allows us to have
/// the mocked Routers pallet (defined above) implementing a Router-like trait
/// (and not just like regular pallet functions) when defining the mocked calls,
/// which is implicitly required by mock-builder or else it fails with `Location
/// must have trait info"`.
trait MockedRouter {
	/// The sender type of the outbound message.
	type Sender;

	type Hash;

	/// Initialize the router.
	fn init() -> DispatchResult;

	/// Send the message to the router's destination.
	fn send(sender: Self::Sender, message: Vec<u8>) -> DispatchResultWithPostInfo;

	fn hash() -> Self::Hash;
}
