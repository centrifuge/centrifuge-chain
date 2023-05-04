// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use codec::Input;
use sp_std::vec::Vec;

/// An encoding & decoding trait for the purpose of meeting the
/// Connectors General Message Passing Format
pub trait Codec: Sized {
	fn serialize(&self) -> Vec<u8>;
	fn deserialize<I: Input>(input: &mut I) -> Result<Self, codec::Error>;
}
