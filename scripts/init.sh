#!/usr/bin/env bash

set -e

cmd=$1
case $cmd in
install-toolchain)
  ./scripts/install_toolchain.sh
  ;;

start-relay-chain)
  echo "Starting local relay chain with Alice and Bob..."
  docker-compose -f ./docker-compose-local-relay.yml up -d
  ;;

stop-relay-chain)
  echo "Stopping relay chain..."
  docker-compose -f ./docker-compose-local-relay.yml down
  ;;

start-parachain)
  echo "Building parachain..."
  cargo build --release
  rm -rf /tmp/centrifuge-chain
  chain="${CHAIN:-charcoal-chachacha-local}"
  ./scripts/run_collator.sh \
    --chain=$chain --alice \
    --base-path=/tmp/centrifuge-chain/data \
    --port 30355 \
    --rpc-port 9936 \
    --ws-port 9946 \
    --rpc-external \
    --rpc-cors all \
    --ws-external \
    --rpc-methods=Unsafe \
    --log="main,info" \
  ;;

onboard-parachain)
  yarn global add @polkadot/api-cli@0.32.1
  chain="${CHAIN:-charcoal-chachacha-local}"
  genesis=$(./target/release/centrifuge-chain export-genesis-state --chain=$chain)
  wasm=$(./target/release/centrifuge-chain export-genesis-wasm --chain=$chain)
  echo "Genesis state:" $genesis
  echo "WASM:" "./target/release/wbuild/centrifuge-chain-runtime/centrifuge_chain_runtime.compact.wasm"

  polkadot-js-api \
          --ws ws://0.0.0.0:9944 \
          --seed "//Alice" \
          --sudo \
          tx.parasSudoWrapper.sudoScheduleParaInitialize \
          2000 \
          "{ \"genesisHead\":\"${genesis?}\", \"validationCode\": \"${wasm}\", \"parachain\": true }"
  ;;
esac
