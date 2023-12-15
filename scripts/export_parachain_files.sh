#!/usr/bin/env bash

set -e

chain_name=$1
parachain_id=$2
should_build=$3

if [[ $chain_name == "" || $parachain_id == "" ]]; then
  echo "Chain Name or Parachain ID argument not provided"
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
  sed -i.bu "s/\"parachainId\": 10001/\"parachainId\": $parachain_id/g" node/res/$chain_name-spec.json
  $PWD/target/release/centrifuge-chain build-spec --chain node/res/$chain_name-spec.json --disable-default-bootnode --raw > node/res/$chain_name-spec-raw.json
  rm node/res/$chain_name-spec.json.bu
fi

echo "Exporting State & Wasm"
$PWD/target/release/centrifuge-chain export-genesis-state --chain node/res/$chain_name-spec-raw.json --parachain-id $parachain_id > $chain_name-genesis-state
$PWD/target/release/centrifuge-chain export-genesis-wasm --chain node/res/$chain_name-spec-raw.json > $chain_name-genesis-wasm
