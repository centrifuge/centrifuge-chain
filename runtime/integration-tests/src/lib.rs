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

// Allow dead code for utilities not used yet
#![allow(dead_code)]
// All code in this crate is test related
#![cfg(test)]

// Allow `#[test_runtimes]` macro to be called everywhere in the crate
#[macro_use]
extern crate runtime_integration_tests_proc_macro;

mod generic;
mod utils;
