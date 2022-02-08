#!/usr/bin/env bash

set -e

cmd=$1
parachain="${PARA_CHAIN_SPEC:-altair-local}"
para_id="${PARA_ID:-2000}"

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
  if [ "$2" == "purge" ]; then
    echo "purging parachain..."
    rm -rf /tmp/centrifuge-chain
  fi

  ./scripts/run_collator.sh \
    --chain="${parachain}" --alice \
    --parachain-id="${para_id}" \
    --base-path=/tmp/centrifuge-chain/data \
    --wasm-execution=compiled \
    --execution=wasm \
    --port 30355 \
    --rpc-port 9936 \
    --ws-port 9946 \
    --rpc-external \
    --rpc-cors all \
    --ws-external \
    --rpc-methods=Unsafe \
    --log="main,debug" \
  ;;

onboard-parachain)
  yarn global add @polkadot/api-cli@0.32.1
  genesis=$(./target/release/centrifuge-chain export-genesis-state --chain="${parachain}" --parachain-id="${para_id}")
  wasm=$(./target/release/centrifuge-chain export-genesis-wasm --chain="${parachain}")
  echo "Genesis state:" $genesis
  echo "WASM:" "./target/release/wbuild/centrifuge-runtime/centrifuge_runtime.compact.wasm"

  polkadot-js-api \
          --ws ws://0.0.0.0:9944 \
          --seed "//Alice" \
          --sudo \
          tx.parasSudoWrapper.sudoScheduleParaInitialize \
          2000 \
          "{ \"genesisHead\":\"${genesis?}\", \"validationCode\": \"${wasm}\", \"parachain\": true }"
  ;;

benchmark)
  ./scripts/run_benchmark.sh "${parachain}" "$2" "$3"
esac
