use crate::mock::*;

#[test]
fn foo() {
	new_test_ext().execute_with(|| {
		assert!(true);
	});
}
