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

start-parachain-docker)
  echo "Starting local parachain with Alice..."
  docker-compose -f ./docker-compose-local-chain.yml up -d
  ;;

stop-parachain-docker)
  echo "Stopping local parachain with Alice..."
  docker-compose -f ./docker-compose-local-chain.yml down
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
  echo "NOTE: This command onboards the parachain; Block production will start in a few minutes"

  genesis=$(./target/release/centrifuge-chain export-genesis-state --chain="${parachain}" --parachain-id="${para_id}")
  wasm_location="${PWD}/${parachain}-${para_id}.wasm"
  ./target/release/centrifuge-chain export-genesis-wasm --chain="${parachain}" > $wasm_location

  echo "Parachain Id:" $para_id
  echo "Genesis state:" $genesis
  echo "WASM path:" "${parachain}-${para_id}.wasm"

  cd scripts/js/onboard
  yarn && yarn execute "//Alice" ${para_id} "${genesis}" $wasm_location
  ;;

benchmark)
  ./scripts/run_benchmark.sh "${parachain}" "$2" "$3"
  ;;
esac
