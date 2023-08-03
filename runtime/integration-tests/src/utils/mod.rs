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

use cfg_types::tokens::CurrencyId;

pub mod accounts;
pub mod env;
pub mod evm;
pub mod extrinsics;
pub mod genesis;
pub mod loans;
pub mod logs;
pub mod pools;
pub mod time;
pub mod tokens;

/// The relay native token's asset id
pub const RELAY_ASSET_ID: CurrencyId = CurrencyId::ForeignAsset(1);
/// The Glimmer asset id
pub const GLIMMER_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1000);
/// The AUSD asset id
pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2000);
/// The EVM Chain id of Moonbea
pub const MOONBEAM_EVM_CHAIN_ID: u64 = 1284;
