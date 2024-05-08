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
use pallet_anchors::AnchorData;
use parity_scale_codec::Codec;
use sp_api::decl_runtime_apis;

decl_runtime_apis! {
	/// Runtime Api for the pallet-anchors, to be implemented
	/// by and for a specific runtime that uses that pallet.
	pub trait AnchorApi<Hash, BlockNumber>
	where
		Hash: Codec,
		BlockNumber: Codec
	{
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
	}
}
