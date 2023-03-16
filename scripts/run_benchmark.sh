#!/usr/bin/env bash

chain=$1
# The pallet name is expected to be the `name` set in the
# respective Cargo.toml, e.g. 'pallet-crowdloan-claim'.
pallet=$2
output=$3

if [  -z "${output}" ]; then
    output=$(echo "./${pallet}/src/weights.rs" | sed 's/pallet-/\pallets\//')
fi

echo "Benchmarking ${pallet} for runtime $chain..."
cargo run --release --features runtime-benchmarks -- benchmark pallet \
  --chain="${chain}" \
  --steps=50 \
  --repeat=20 \
  --pallet="${pallet}" \
  --extrinsic=* \
  --execution=wasm \
  --wasm-execution=compiled \
  --heap-pages=4096 \
  --template=./scripts/frame-weight-template.hbs \
  --output="${output}"

# since benchmark generates a weight.rs file that may or may not cargo fmt'ed.
# so do cargo fmt here.
cargo fmt
echo "Benchmarked weights are written to ${output}"
