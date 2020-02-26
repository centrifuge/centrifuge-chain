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
./scripts/init.sh
```

Build Wasm and native code:

```bash
cargo build --release
```

## Run

### Tests

```bash
cargo test --release
```

### Testnets

Centrifuge has multiple testnets online.

1. Fulvous is an ephemeral testnet for internal dev purposes, and testing internal integrations with all the centrifuge components. Not recommended for external usage due to its nature and purpose.
2. Flint is for breaking changes and testing the integration with other parts of the Centrifuge ecosystem. Think of Flint as a way to test previews, alpha releases.
3. Amber is for audits and testing of the stability of release candidates. Think of Amber as a way to test beta releases.

#### 1. Fulvous

To run a node:

```bash
./target/release/centrifuge-chain \
  --chain=fulvous \
  --name "My node name" \
  --bootnodes /ip4/35.246.140.178/tcp/30333/p2p/QmRg2bEPTHCt8u3a1LeZA8dJTd8mgMccsAcoHXTjQUpcZj \
  --bootnodes /ip4/35.198.166.26/tcp/30333/p2p/QmNpeu3bJhESzriWMLRcxRgSCYDGQ6GdBHnJAf8bJexAd5
```

#### 2. Flint

To run a node:

```bash
./target/release/centrifuge-chain \
  --chain=flint \
  --name "My node name" \
  --bootnodes=/ip4/34.89.190.227/tcp/30333/p2p/QmdMJoLc6yduqfrJtMAB6xHegydr3YXzfDCZWEYsaCJaRZ \
  --bootnodes=/ip4/35.234.68.18/tcp/30333/p2p/Qma5M7P5qym3Gfgp1wu6yk1QyMv2RzFV9GztP9AxHoK8PK \
  --bootnodes=/ip4/35.246.244.114/tcp/30333/p2p/QmdjEGZ9ZNVv4aTGGV46AkBqgCdWTHrh9wr9itYhs61gJA \
  --bootnodes=/ip4/34.89.148.219/tcp/30333/p2p/QmNd8inSbEvFuwbRToj5VQBNReqtb414oWGyDjF7tQ1qfX
```

To receive tokens, use our faucet: https://faucets.blockxlabs.com/

To run a validator: https://centrifuge.hackmd.io/@pstehlik/rJ4ldDdiH

#### 2. Amber

tbd

### Single node development chain

Purge any existing developer chain state:

```bash
cargo run --release -- purge-chain --dev
```

Start a development chain with:

```bash
cargo run --release -- --dev
```

Detailed logs may be shown by running the node with the following environment variables set: `RUST_LOG=debug RUST_BACKTRACE=1 cargo run --release -- --dev`.

### Multi-node local testnet

If you want to see the multi-node consensus algorithm in action locally, then you can create a local testnet with two validator nodes for Alice and Bob, who are the initial authorities that have been endowed with testnet tokens.

You'll need two terminal windows open.

We'll start Alice's node first on default TCP port 30333 with her chain database stored locally at `/tmp/alice`. The identity of her node is `QmPf2cdiE6Sp2Njxzy6cz8vHA7ii86mMFF61e6NMGRtFbr`:

```bash
./target/release/centrifuge-chain \
  --base-path /tmp/alice \
  --chain=local \
  --alice
```

In the second terminal, we'll start Bob's node on TCP port 30334, and with his chain database stored locally at `/tmp/bob`. We'll specify a value for the `--bootnodes` option that will connect his node to Alice's bootnode ID on TCP port 30333:

```bash
./target/release/centrifuge-chain \
  --base-path /tmp/bob \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/QmPf2cdiE6Sp2Njxzy6cz8vHA7ii86mMFF61e6NMGRtFbr \
  --chain=local \
  --bob \
  --port 30334
```

Additional CLI usage options are available and may be shown by running `./target/release/centrifuge-chain --help`.

### Generating a new genesis file

1. Be sure to change the `id` and `protocol_id` in `src/chain_spec.rs`
2. Run `cargo run --release build-spec --disable-default-bootnode --chain flint > res/[name]-spec.json` to export the chain spec
3. Commit

## Linting

Lint the project with `cargo +nightly fmt`. This excludes certain paths (defined in `rustfmt.toml`) that we want to stay as close as possible to `paritytech/substrate` to simplify upgrading to new releases.
