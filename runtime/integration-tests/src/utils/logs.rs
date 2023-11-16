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

//! Utilities to initialize logging subscriber
use std::sync::atomic::{AtomicUsize, Ordering};

use tracing_subscriber::filter::LevelFilter;

static GLOBAL_INIT: AtomicUsize = AtomicUsize::new(UNINITIALIZED);

const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

const LOG_LEVEL: LevelFilter = LevelFilter::ERROR;

pub fn init_logs() {
	if GLOBAL_INIT
		.compare_exchange(
			UNINITIALIZED,
			INITIALIZING,
			Ordering::SeqCst,
			Ordering::SeqCst,
		)
		.is_ok()
	{
		GLOBAL_INIT.store(INITIALIZED, Ordering::SeqCst);
		tracing_subscriber::fmt::fmt()
			.with_max_level(LOG_LEVEL)
			.init();
	}
}
