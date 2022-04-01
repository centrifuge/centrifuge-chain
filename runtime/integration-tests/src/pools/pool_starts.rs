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
use crate::chain::centrifuge;
use crate::chain::centrifuge::{Runtime, PARA_ID};
use crate::pools::utils::accounts::Keyring;
use crate::pools::utils::extrinsics::ext_centrifuge;
use crate::pools::utils::*;
use codec::Encode;
use fudge::primitives::Chain;
use pallet_balances::Call as BalancesCall;
use sp_runtime::Storage;
use tokio::runtime::Handle;

#[tokio::test]
async fn create_pool() {}
