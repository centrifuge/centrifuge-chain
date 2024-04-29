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

fn main() {
	// FIXME: Pathing seems off
	// TODO: log current directory
	let paths = fs::read_dir("./submodules/")
		.expect("Submodules directory must exist for integration-tests");
	let out_dir = env::var("OUT_DIR").expect("Cargo sets OUT_DIR environment variable. qed.");

	let current_dir = env::current_dir().expect("Current dir exists");
	println!("Current directory is {current_dir:?}");

	let files_in_cur_dir = fs::read_dir("./").expect("Current directory exists");
	println!("Files in current directory are {files_in_cur_dir:?}");

	let mut verified_dir = Vec::new();
	for path in paths {
		if let Ok(dir_entry) = path.as_ref() {
			if dir_entry
				.metadata()
				.map(|meta| meta.is_dir())
				.unwrap_or(false)
			{
				verified_dir.push(
					fs::canonicalize(dir_entry.path()).expect("Failed to find absolute path."),
				);
			}
		}
	}

	for path in verified_dir {
		env::set_current_dir(&path).unwrap();
		let mut out_dir_build = PathBuf::from(out_dir.clone());

		let parent = path
			.components()
			.last()
			.expect("Repository itself has components. qed")
			.as_os_str()
			.to_str()
			.expect("OsStr is utf-8. qed");

		out_dir_build.push(parent);
		let out_dir_build = out_dir_build
			.as_os_str()
			.to_str()
			.expect("OsStr is utf-8. qed");

		match Command::new("forge")
			.args(["build", "--out", out_dir_build])
			.output()
		{
			Ok(o) if o.status.success() => {
				let source = match parent {
					"liquidity-pools" => {
						println!(
							"cargo:info=Build liquidity-pools Solidity contracts. Stored at {} ",
							LP_SOL_SOURCES
						);

						LP_SOL_SOURCES
					}
					_ => {
						println!(
							"cargo:warning=Unknown solidity source build. Name: {}",
							parent
						);
						println!(
                            "cargo:warning=No environment variable for sources set. Artifacts stored under: {}",
                            out_dir_build
                        );
						continue;
					}
				};

				println!("cargo:rustc-env={}={}", source, out_dir_build);
			}
			Ok(o) => {
				println!(
					"cargo:warning=forge build failed with: \n  - status: {}\n   -stderr: {}",
					o.status,
					String::from_utf8(o.stderr).expect("stderr is utf-8 encoded. qed.")
				);
			}
			Err(err) => {
				println!("Current directory is {current_dir:?}");
				println!("Files in current directory are {files_in_cur_dir:?}");
				println!("cargo:warning=Failed to instantiate the submodule: {}", err);
			}
		}
	}
}
