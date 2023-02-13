use cfg_traits::PoolReserve;
use frame_support::assert_ok;

use super::{mock::*, *};

#[test]
fn wrong_test_example() {
	new_test_ext().execute_with(|| {
		MockPools::expect_withdraw(|_, _, amount| {
			assert_eq!(amount, 999);
			Ok(())
		});

		assert_ok!(MockPools::withdraw(1, 2, 1000));
	});
}

#[test]
fn any_fn() {
	use std::any::Any;

	struct Callback<Args, R>(Box<dyn Fn(Args) -> R>);

	trait Callable {
		fn as_any(&self) -> &dyn Any;
	}

	impl<Args: 'static, R: 'static> Callable for Callback<Args, R> {
		fn as_any(&self) -> &dyn Any {
			self
		}
	}

	let t = Callback::<(), ()>(Box::new(|_| println!("aaa")));
	let x: &dyn Callable = &t;

	let p: &Callback<(), ()> = x.as_any().downcast_ref().unwrap();
	p.0(());
}
