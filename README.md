# centrifuge-chain

[![Build Status](https://travis-ci.com/centrifuge/centrifuge-chain.svg?branch=master)](https://travis-ci.com/centrifuge/centrifuge-chain)
[![codecov](https://codecov.io/gh/centrifuge/centrifuge-chain/branch/master/graph/badge.svg)](https://codecov.io/gh/centrifuge/centrifuge-chain)

Centrifuge Chain is [Centrifuge](https://centrifuge.io)'s [substrate](https://github.com/paritytech/substrate) based chain.

## Build

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Initialize your Wasm Build environment:

```bash
./scripts/init.sh install-toolchain
```

Build Wasm and native code:

```bash
cargo build --release
```

## Run

### Tests

```bash
cargo test -p centrifuge-runtime --release
```

### Start local Relay chain(alice and bob) and Parachain(alice)  

Start relay chain
```bash
./scripts/init.sh start-relay-chain
```

Start  centrifuge-chain as parachain
```bash
./scripts/init.sh start-parachain
```

Note: above command will show logs and block until parachain is stopped  
Detailed logs may be shown by running the node with the following environment variables set: `RUST_LOG=debug RUST_BACKTRACE=1`.

Onboard parachain to Relay chain
```bash
./scripts/init.sh onboard-parachain
```

### Generating a new genesis file

1. Be sure to change the `id` and `protocol_id` in `src/chain_spec.rs`
2. Run `cargo run --release build-spec --disable-default-bootnode --chain fulvous > res/[name]-spec.json` to export the chain spec
3. Commit

## Linting

Lint the project with `cargo +nightly fmt`. This excludes certain paths (defined in `rustfmt.toml`) that we want to stay as close as possible to `paritytech/substrate` to simplify upgrading to new releases.

## Verifying Runtime
1. Check out the commit at which the runtime was built.
2. Run `TARGET=build-runtime RUST_TOOLCHAIN=nightly ./ci/script.sh`
3. A similar output is generated
```
✨ Your Substrate WASM Runtime is ready! ✨
Summary:
  Generator  : srtool v0.9.5
  GIT commit : 27326e69481f08313d6048da1500befe209bdf71
  GIT tag    : v0.0.3
  GIT branch : master
  Time       : 2020-03-20T11:00:24Z
  Rustc      : rustc 1.43.0-nightly (5e7af4669 2020-02-16)
  Size       : 928 KB (950464 bytes)
  Content    : 0x0061736d0100000001c2022f60037f7f...3436363920323032302d30322d313629
  Package    : centrifuge-chain-runtime
  Proposal   : 0x5c3d2cd41d70c514566c9b512743ad229fa96518061fe21c8178ba43cfcf16dc
  SHA256     : 3f0d2e98e2351144027826f26277bda90e5fabc13f0945fc8fec13d116602e2a
  Wasm       : ./target/srtool/release/wbuild/centrifuge-chain-runtime/centrifuge_chain_runtime.compact.wasm
```
4. `Proposal` hash should match the runtime upgrade proposal

## Upgrading Substrate, Polkadot, and Grandpa bridge gadget
1. Pull the latest commit of `cumulus` that is building without issues.
2. Then take the commits of `substrate, polkadot, and grandap-bridge-gadget` from Cargo.lock of cumulus.
3. Move our substrate fork `master` branch to commit you derived above.
4. Then for each repo in the order `grandpa-bridge-gadget, polkadot, and cumulus`, 
   move our fork's `master` branch to the commit derived above and rebase those on `centrifuge` branch
5. Then on centrifuge, deleting Cargo.lock file and running `cargo check`  will pull the latest commits from respective forks 

## Generate new Spec and Parachain files
This script will take a valid chain-spec chain_id, a parachain_id and a flag to build new spec or not, and will output genesis spec (raw and plain), wasm and state files.
```shell
./scripts/export_parachain_files.sh charcoal-staging 10001 true
```
Adapt parameters accordingly.

## Benchmarking pallets
Pallets are to be benchmarked to find the correct weight for extrinsics. Follow substrate's benchmarking boiler-plate code
and add pallet benchmark to the runtime. Then run the following script to generate a benchamarked `weights.rs` file for the pallet
```shell
./scripts/init.sh benchmark <your_pallet> <output generated weight file>(optional)
```

Example command to generate `pallet_fees` with default `output`
```shell
./scripts/init.sh benchmark pallet_fees
```

default output will be `./pallets/fees/src/weight.rs`
You can override this by passing output path as last argument

## Upgrading to latest cumulus(until they have tags for releases)
1. First collect commits of Substrate, Grandpa-Bridge, Polkadot from the latest cumulus
2. Bring our fork of all the above repos to the above commits
3. Use diener in Grandpa brigde, polkadot, and cumulus to use our forks in `centrifuge` branch
4. Then update deps on centrifuge chain
