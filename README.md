# centrifuge-chain

[![Build Status](https://travis-ci.com/centrifuge/centrifuge-chain.svg?branch=master)](https://travis-ci.com/centrifuge/centrifuge-chain)
[![codecov](https://codecov.io/gh/centrifuge/centrifuge-chain/branch/master/graph/badge.svg)](https://codecov.io/gh/centrifuge/centrifuge-chain)

The Centrifuge Chain is [Centrifuge](https://centrifuge.io)'s [Substrate](https://github.com/paritytech/substrate)-based chain.

## Build

Install [Rust](https://www.rust-lang.org/tools/install):

Initialize your Wasm Build environment:

```bash
./scripts/init.sh install-toolchain
```

Build Wasm and native code:

```bash
cargo build --release
```

Great! You have already compile the Centrifuge Chain!

## Tests

There are two kinds of tests, one related to how the *Centrifuge Chain* works itself
and another one to verify how it works in a more real environment as a parachain.

### Chain tests

The following command will the unit and the integration tests:

```bash
cargo +nightly test --workspace --release --features test-benchmarks,try-runtime
```

### Environment tests

You can deploy a relay chain and connect a Centrifuge Chain node as parachain
to it to verify how it behaves in the entire environment (end-to-end).

0. Prerequisites. You must install these tools before:
    - [docker](https://docs.docker.com/get-docker/)
    - [*jd*](https://stedolan.github.io/jq/)

1. Start a local [relay chain](https://wiki.polkadot.network/docs/learn-architecture#relay-chain).
It contains two [validator](https://wiki.polkadot.network/docs/learn-validator) nodes
    (Alice and Bob):
    ```bash
    ./scripts/init.sh start-relay-chain
    ```
    After a few seconds you can see the block production of the relay chain using the [polkadot.js (on localhost:9944)](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2Flocalhost%3A9944#/explorer) client.

    *Note: You can stop the relay chain using `./scripts/init.sh stop-relay-chain`*

2. Start a *Centrifuge Chain* as [parachain](https://wiki.polkadot.network/docs/learn-parachains).
It run a [collator](https://wiki.polkadot.network/docs/learn-collator) node:
    ```bash
    ./scripts/init.sh start-parachain
    ```
    *Note: the command above will show logs and block until the parachain is stopped.
    If you had a previous state, you can reset the node using `purge` after the command.*

    Similar to the relay chain, you can explore the parachain using the [polkadot.js (on localhost:11946)](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2Flocalhost%3A11946#/explorer) client.
    You will see the block production frozen until you connect it to the relay chain.

    By default, the initialized parachain will have the id `2000`.
    If you need more than one parachain or choose other chain specifications,
    you can set `PARA_ID` or `PARA_CHAIN_SPEC`, example:
    ```bash
    PARA_ID=2001 ./scripts/init.sh start-parachain
    ```
    The different `PARA_CHAIN_SPEC` values can be found at [`src/command.rs`](src/command.rs) under the `load_spec()` function.

3. Onboard the parachain
    This step will have the targeted parachain onboarded in the relay chain. The parachain will NOT produce blocks until this step is completed successfully.
    ```bash
    ./scripts/init.sh onboard-parachain
    ```
    When you have run the command, you should see in the relay-chain dashboard that there is a parachain
    that will be onboarded in one/two minutes.
    Once onboarded, block production should start soon after in the parachain.

That's all! The environment is set.
You can play with it from the parachain client, make transfers, inspect events, etc.

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
  Package    : centrifuge-runtime
  Proposal   : 0x5c3d2cd41d70c514566c9b512743ad229fa96518061fe21c8178ba43cfcf16dc
  SHA256     : 3f0d2e98e2351144027826f26277bda90e5fabc13f0945fc8fec13d116602e2a
  Wasm       : ./target/srtool/release/wbuild/centrifuge-runtime/centrifuge_runtime.compact.wasm
```
4. `Proposal` hash should match the runtime upgrade proposal

## Generate new Spec and Parachain files
This script will take a valid chain-spec chain_id, a parachain_id and a flag to build new spec or not, and will output genesis spec (raw and plain), wasm and state files.
```shell
./scripts/export_parachain_files.sh charcoal-staging 10001 true
```
Adapt parameters accordingly.


## Benchmarking

### Benchmarking pallets
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

### Benchmarking runtimes

When benchmarking pallets, we are just running the benchmarking scenarios they specify
within their mocked runtime. This fails to actually benchmark said pallet in the context
in which it will be actually used in production: within a specific runtime and composing
with other pallets.

To cover that, we run test for every pallet for a given runtime and use the output weights
in production since those are the most trustworthy weights we can use.

Note: This command should be run in a cloud environment that mimics closely the specs of
the collator node the parachain will be running on.

```shell
./scripts/runtime_benchmarks.sh <runtime>
```

# Upgrading to a newer Polkadot / Substrate / Cumulus version

When a new version of Polkadot is released, a mirroring release happens for the other
parity projects such as Substrate and Cumulus, but also for other third-party projects
such as the `orml` pallets, `xcm-simulator`, etc.

You can follow these steps tailored to update only one of those specific projects
updating the three simultaneously is the most common flow.

The flow to upgrade to a newer version of polkadot is usually as follows:

1. Use [`diener`](https://github.com/bkchr/diener) to update Polkadot, Substrate, and Cumulus to said release

```shell
export POLKADOT_NEW_VERSION="<version>"; # for example, 0.9.24

diener update --polkadot --branch release-v$POLKADOT_NEW_VERSION;
diener update --substrate --branch polkadot-v$POLKADOT_NEW_VERSION;
diener update --cumulus --branch polkadot-v$POLKADOT_NEW_VERSION;
```

2. Repeat step 1 for our other repos that we depend on:

- [centrifuge/chainbridge-substrate](https://github.com/centrifuge/chainbridge-substrate)
- [centrifuge/unique-assets](https://github.com/centrifuge/unique-assets)
- [centrifuge/fudge](https://github.com/centrifuge/fudge)

3. Make sure that we point to the latest revision of the projects altered in step 2

Note: If pointing to `branch = "master"`, run `Cargo update`

4. Repeat step 3 for other third-party dependencies that also depend on Polkadot/Substrate/Cumulus

- [`orml` pallets](https://github.com/open-web3-stack/open-runtime-module-library) - All of them
- [xcm-simulator](https://github.com/shaunxw/xcm-simulator)
- potentially more

Note: if any of the project do not yet have a cut for the target polkadot release, either wait or
fork said project and run step 1 for it and open a PR and point that revision.

5. Build and test the project and migrate any introduced change

Tip: if you face compilation errors like "type X doesn't implement trait Y", and the compiler
doesn't suggest you import any particular trait, experience is that there are different versions
of Polkadot|Substrate|Cumulus being pulled; Look at `Cargo.lock` and find which project is still
depending on an older version of any of those projects.
