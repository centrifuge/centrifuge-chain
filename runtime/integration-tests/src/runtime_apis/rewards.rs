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

use altair_runtime::apis::AnchorApi;
use tokio::runtime::Handle;

use super::ApiEnv;

#[tokio::test]
async fn test() {
	ApiEnv::new(Handle::current())
		.startup(|| {
			// Code used in rewards GenesisConfig
			// to actually set-up the state you need
		})
		.with_api(|api, latest| {
			// Do actually call your api. Using anchor here for the sake of an example
			//
			// First argument, is expanded by the macro to be the block to call the api at.
			// The env simply passes on the latest block. We could also have a macro
			// proxying the api calls, but kinda big overhead.
			let _ = api.get_anchor_by_id(&latest, Default::default());
		});
}
