// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Utilities around the dispatching calls
use frame_support::{assert_ok, dispatch::UnfilteredDispatchable};

use crate::chain::centrifuge::RuntimeOrigin;

/// A helper macro which is using the trait `UnfilteredDispatchable`
/// (https://paritytech.github.io/substrate/master/frame_support/dispatch/trait.UnfilteredDispatchable.html
/// From the docs: "Type that can be dispatched with an origin but without checking the origin filter."
///
/// We use this to execute our pallet calls in the integration tests. We use the utils to create calls of the
/// type `RuntimeCall`, and pass them to the `dispatch_bypass_filter` function. This macro saves us
/// from exposing the `UnfilteredDispatchable` trait implementation to each test where we might need it
/// and reduce the cognitive overload this might cause.
macro_rules! dispatch {
	($calls:expr, $account:expr) => {
		for call in $calls {
			let res = UnfilteredDispatchable::dispatch_bypass_filter(
				call,
				RuntimeOrigin::signed($account),
			);
			assert_ok!(res);
		}
	};
}

pub(crate) use dispatch;
