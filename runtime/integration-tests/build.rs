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
use std::{env, fs, path::PathBuf, process::Command};

const LP_SOL_SOURCES: &str = "LP_SOL_SOURCES";
const VAULTS_SOL_SOURCES: &str = "VAULTS_SOL_SOURCES";

fn main() {
	// Added debug message to confirm the script is running.
	#[cfg(feature = "debug-evm")]
	println!("cargo:warning=build.rs started");

	let submodules_path = "./submodules/";
	#[cfg(feature = "debug-evm")]
	println!(
		"cargo:warning=Looking for submodules in: {}",
		submodules_path
	);

	let paths = fs::read_dir(submodules_path)
		.expect("Submodules directory must exist for integration-tests");

	let out_dir = env::var("OUT_DIR").expect("Cargo sets OUT_DIR environment variable. qed.");
	#[cfg(feature = "debug-evm")]
	println!("cargo:warning=OUT_DIR is set to: {}", out_dir);

	let mut verified_dir = Vec::new();
	for path in paths {
		match path {
			Ok(dir_entry) => {
				if dir_entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
					let canonical = fs::canonicalize(dir_entry.path())
						.expect("Failed to determine canonical path.");

					#[cfg(feature = "debug-evm")]
					println!("cargo:warning=Found submodule directory: {:?}", canonical);
					verified_dir.push(canonical);
				}
			}
			Err(e) => {
				println!(
					"cargo:warning=Error reading submodules directory entry: {}",
					e
				);
			}
		}
	}

	for path in verified_dir {
		// Change working directory.
		if let Err(e) = env::set_current_dir(&path) {
			println!(
				"cargo:warning=Failed to set current dir to {:?}: {}",
				path, e
			);
			std::process::exit(1);
		}
		let mut out_dir_build = PathBuf::from(&out_dir);

		// Extract the submodule folder name.
		let parent = path
			.components()
			.last()
			.expect("Directory has no components?")
			.as_os_str()
			.to_str()
			.expect("Directory name is not valid UTF-8");

		#[cfg(feature = "debug-evm")]
		println!("cargo:warning=Processing submodule: {}", parent);

		// Append the name to the build output directory.
		out_dir_build.push(parent);
		let out_dir_build = out_dir_build
			.as_os_str()
			.to_str()
			.expect("OUT_DIR build path is not valid UTF-8");

		#[cfg(feature = "debug-evm")]
		{
			println!(
				"cargo:warning=Output directory for {}: {}",
				parent, out_dir_build
			);
			println!("cargo:warning=Out dir build: {out_dir_build:?}");
		}

		match Command::new("forge")
			.args(["build", "--out", out_dir_build])
			.output()
		{
			Ok(o) if o.status.success() => {
				#[cfg(feature = "debug-evm")]
				println!("cargo:warning=forge build succeeded for {}", parent);

				let source = match parent {
					"liquidity-pools" => {
						println!(
							"cargo:warning=Built liquidity-pools Solidity contracts stored at {}",
							out_dir_build
						);
						LP_SOL_SOURCES
					}
					"vaults-internal" => {
						println!(
							"cargo:warning=Built vaults-internal Solidity contracts stored at {}",
							out_dir_build
						);
						VAULTS_SOL_SOURCES
					}
					_ => {
						println!("cargo:warning=Unknown solidity source: {}", parent);
						println!(
							"cargo:warning=Skipping environment variable setting. Artifacts stored at {}",
							out_dir_build
						);
						continue;
					}
				};

				println!("cargo:rustc-env={}={}", source, out_dir_build);
			}
			Ok(o) => {
				println!(
					"cargo:warning=forge build failed with: \n  - status: {:?}\n   -stderr: {:?}",
					o.status,
					String::from_utf8(o.stderr).unwrap_or_else(|_| "Non UTF-8 output".into())
				);
			}
			Err(err) => {
				println!(
					"cargo:warning=Failed to execute forge command for {}: {}",
					parent, err
				);
			}
		}
	}
}
