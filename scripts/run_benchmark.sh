#!/usr/bin/env bash

set -eux

chain=$1
pallet=$2
output=$3
if [  -z "${output}" ]; then
    output=$(echo "./${pallet}/src/weights.rs" | sed 's/_/\//')
    output=$(echo "${output}" | sed 's/pallet\//pallets\//')
fi

echo "Benchmark: ${pallet}"
cargo +nightly run --release --features runtime-benchmarks -- benchmark \
  --chain="${chain}" \
  --steps=50 \
  --repeat=100 \
  --pallet="${pallet}" \
  --extrinsic=* \
  --execution=wasm \
  --wasm-execution=compiled \
  --heap-pages=4096 \
  --output="${output}" \
  --template=./scripts/frame-weight-template.hbs

# since benchmark generates a weight.rs file that may or may not cargo fmt'ed.
# so do cargo fmt here.
cargo fmt
