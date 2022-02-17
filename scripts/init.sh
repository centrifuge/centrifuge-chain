#!/usr/bin/env bash

set -e

cmd=$1
# The runtime we want to use
parachain="${PARA_CHAIN_SPEC:-altair-local}"
# The parachain Id we want to use
para_id="${PARA_ID:-2000}"
# The parachain path for data storage
parachain_dir=/tmp/centrifuge-chain-${para_id}

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
  printf "\nBuilding parachain with runtime '$parachain' and id '$para_id'...\n"
  cargo build --release

  if [ "$2" == "purge" ]; then
    echo "purging parachain..."
    rm -rf $parachain_dir
  fi

  ./scripts/run_collator.sh \
    --chain="${parachain}" --alice \
    --parachain-id="${para_id}" \
    --base-path=$parachain_dir/data \
    --wasm-execution=compiled \
    --execution=wasm \
    --port $((30355 + $para_id)) \
    --rpc-port $((9936 + $para_id)) \
    --ws-port $((9946 + $para_id)) \
    --rpc-external \
    --rpc-cors all \
    --ws-external \
    --rpc-methods=Unsafe \
    --state-cache-size 0 \
    --log="main,debug" \
  ;;

onboard-parachain)
  echo "NOTE: This command does NOT onboard the parachain; It only outputs the required parameters for the parachain to be onboarded."

  genesis=$(./target/release/centrifuge-chain export-genesis-state --chain="${parachain}" --parachain-id="${para_id}")
  # Extract the runtime id from $parachain.
  # For example, 'development-local' becomes 'development', 'altair-local' becomes 'altair', etc.
  runtime_id=$(echo $parachain | cut -d'-' -f 1)

  echo "Parachain Id:" $para_id
  echo "Genesis state:" $genesis
  echo "WASM path:" "./target/release/wbuild/${runtime_id}-runtime/${runtime_id}_runtime.compact.wasm"
  ;;

benchmark)
  ./scripts/run_benchmark.sh "${parachain}" "$2" "$3"
esac
