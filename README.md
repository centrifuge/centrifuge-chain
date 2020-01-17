# centrifuge-chain

[![Build Status](https://travis-ci.com/centrifuge/centrifuge-chain.svg?branch=master)](https://travis-ci.com/centrifuge/centrifuge-chain)
[![codecov](https://codecov.io/gh/centrifuge/centrifuge-chain/branch/master/graph/badge.svg)](https://codecov.io/gh/centrifuge/centrifuge-chain)

Centrifuge-chain is a substrate based chain.

## Build

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Initialize your Wasm Build environment:

```bash
./scripts/init.sh
```

Build Wasm and native code:

```bash
cargo build --release
```

## Run

### Single node development chain

Purge any existing developer chain state:

```bash
./target/release/centrifuge-chain purge-chain --dev
```

Start a development chain with:

```bash
./target/release/centrifuge-chain --dev
```

Detailed logs may be shown by running the node with the following environment variables set: `RUST_LOG=debug RUST_BACKTRACE=1 cargo run -- --dev`.

### Multi-node local testnet

If you want to see the multi-node consensus algorithm in action locally, then you can create a local testnet with two validator nodes for Alice and Bob, who are the initial authorities of the genesis chain that have been endowed with testnet units.

Optionally, give each node a name and expose them so they are listed on the Polkadot [telemetry site](https://telemetry.polkadot.io/#/Local%20Testnet).

You'll need two terminal windows open.

We'll start Alice's substrate node first on default TCP port 30333 with her chain database stored locally at `/tmp/alice`. The bootnode ID of her node is `QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR`, which is generated from the `--node-key` value that we specify below:

```bash
cargo run -- \
  --base-path /tmp/alice \
  --chain=local \
  --alice \
  --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
  --telemetry-url ws://telemetry.polkadot.io:1024 \
  --validator
```

In the second terminal, we'll start Bob's substrate node on a different TCP port of 30334, and with his chain database stored locally at `/tmp/bob`. We'll specify a value for the `--bootnodes` option that will connect his node to Alice's bootnode ID on TCP port 30333:

```bash
cargo run -- \
  --base-path /tmp/bob \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR \
  --chain=local \
  --bob \
  --port 30334 \
  --telemetry-url ws://telemetry.polkadot.io:1024 \
  --validator
```

Additional CLI usage options are available and may be shown by running `cargo run -- --help`.

### Fulvous

To generate the chain spec,
`cargo run -- build-spec --chain=fulvous > testnets/fulvous.json`

To generate the raw chain spec,
`cargo run -- build-spec --chain=fulvous --raw > testnets/fulvous.raw.json`


### Running locally
For Fulvous

#### Without using genesis file,

Validator Bob:
`./target/debug/centrifuge-chain --ws-external --validator --node-key=66ef62065cfdc48929b5cb9c1bbc0a728e6d1d43b4ba1de13ccf76c7ecec66e9 --rpc-cors=all --chain=fulvous --base-path /tmp/tbob`

Validator Alice (Pass libp2p address of Bob's node above as the bootnode here)
`./target/debug/centrifuge-chain --ws-external --validator --node-key=2a654a0958cd0e10626c36057c46a08018eaf2901f9bab74ecc1144f714300ac --rpc-cors=all --chain=fulvous --base-path /tmp/talice --bootnodes=/ip4/127.0.0.1/tcp/30333/p2p/QmNpeu3bJhESzriWMLRcxRgSCYDGQ6GdBHnJAf8bJexAd5 --rpc-port=9935 --ws-port=9945`

#### Using the genesis file

Validator Bob:
`./target/debug/centrifuge-chain --ws-external --validator --node-key=66ef62065cfdc48929b5cb9c1bbc0a728e6d1d43b4ba1de13ccf76c7ecec66e9 --bob --rpc-cors=all --chain=testts/fulvous.raw.json --base-path /tmp/tbob`

Validator Alice (Pass libp2p address of bobs node above as the bootnode here)
`./target/debug/centrifuge-chain --ws-external --validator --node-key=2a654a0958cd0e10626c36057c46a08018eaf2901f9bab74ecc1144f714300ac --alice --rpc-cors=all --chain=testnets/fulvous.raw.json --base-path /tmp/talice --bootnodes=/ip4/127.0.0.1/tcp/30333/p2p/QmNpeu3bJhESzriWMLRcxRgSCYDGQ6GdBHnJAf8bJexAd5 --port=30334`

### Generating a new genesis file

1. Be sure to change the `id` and `protocol_id` in `src/chain_spec.rs`
2. Run `cargo run --release build-spec --disable-default-bootnode --chain flint > res/[name]-spec.json` to export the chain spec
3. Commit