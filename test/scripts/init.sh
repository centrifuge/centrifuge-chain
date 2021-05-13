#!/usr/bin/env bash

set -e

cargo build --release
rm -rf /tmp/centriug-chain

# export genesis state
mkdir -p /tmp/centrifuge-chain/{genesis,runtime}
./target/release/centrifuge-chain export-genesis-state --chain=charcoal-chachacha-local /tmp/centrifuge-chain/genesis/genesis-state

cp ./target/release/wbuild/centrifuge-chain-runtime/centrifuge_chain_runtime.compact.wasm /tmp/centrifuge-chain/runtime/

yarn global add @polkadot/api-cli@0.32.1

#polkadot-js-api \
#        --ws ws://0.0.0.0:9944 \
#        --seed "//Alice" \
#        tx.registrar.register \
#            10001 \
#            "$(cat /tmp/centrifuge-chain/genesis/genesis-state)" \
#            @/tmp/centrifuge-chain/runtime/centrifuge_chain_runtime.compact.wasm


# run collator
./test/scripts/run_collator.sh \
  --chain=charcoal-chachacha-local --alice \
  --base-path=/tmp/centriuge-chain/data \
  --port 30355 \
  --rpc-port 9936 \
  --ws-port 9946 \
  --rpc-external \
  --rpc-cors all \
  --ws-external \
  --rpc-methods=Unsafe \
  --log="main,info" \
