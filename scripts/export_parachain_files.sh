#!/usr/bin/env bash

set -e

chain_name=$1
should_build=$2

if [[ $chain_name == "" ]]; then
  echo "Chain Name argument not provided"
  exit 1
fi

FILE=./target/release/centrifuge-chain
if [ ! -x "$FILE" ]; then
    echo "FATAL: $FILE does not exist, or not executable, rebuild binary to continue"
    exit 1
fi

if [[ $should_build == "true" ]]; then
  echo "Building Spec for $chain_name"
  $PWD/target/release/centrifuge-chain build-spec --chain $chain_name --disable-default-bootnode > node/res/$chain_name-spec.json
  sed -i.bu "s/\"parachainId\": 2000/" node/res/$chain_name-spec.json
  $PWD/target/release/centrifuge-chain build-spec --chain node/res/$chain_name-spec.json --disable-default-bootnode --raw > node/res/$chain_name-spec-raw.json
  rm node/res/$chain_name-spec.json.bu
fi

echo "Exporting State & Wasm"
$PWD/target/release/centrifuge-chain export-genesis-head --chain node/res/$chain_name-spec-raw.json > $chain_name-genesis-state
$PWD/target/release/centrifuge-chain export-genesis-wasm --chain node/res/$chain_name-spec-raw.json > $chain_name-genesis-wasm
