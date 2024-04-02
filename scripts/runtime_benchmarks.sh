#!/usr/bin/env bash

set -u

runtime=$1

check() {
  return_code=$1

  if [[ $return_code -ne 0 ]]
  then
    echo "Error: Intermediate error occurred. Cleaning up artifacts in '/tmp/${runtime_path}'"
    rm -rf "/tmp/${runtime_path}"
    echo "Aborting!"
    exit 1
  fi
}

run_benchmark() {
  pallet=$1
  output=$2

  cmd="target/release/centrifuge-chain benchmark pallet \
    --chain="${chain}" \
    --steps=50 \
    --repeat=20 \
    --pallet="${pallet}" \
    --extrinsic=* \
    --wasm-execution=compiled \
    --heap-pages=4096 \
    --output="${output}""

    echo "Running benchmark for pallet '${pallet}'"
    echo "${cmd}"
    ${cmd}
}

echo "Benchmarking pallets for runtime ${runtime}..."

# Find the correct path to create weights in
if [[ $runtime == "development" ]];
then
  runtime_path="runtime/development"
  chain="development-local"
elif [[ $runtime == "centrifuge" ]];
then
  runtime_path="runtime/centrifuge"
  chain="centrifuge-dev"
elif [[ $runtime == "altair" ]];
then
  runtime_path="runtime/altair"
  chain="altair-dev"
else
  echo "Unknown runtime. Aborting!"
  exit 1;
fi

# Ensure this script is started in the root directory of the repository
path_from_root="${PWD}/scripts/runtime_benchmarks.sh"
if [[ -f ${path_from_root} ]];
then
  echo ""
else
  echo "Runtime benchmark script not started from expected root of './scripts/runtime_benchmarks.sh'"
  echo "Aborting!"
  exit 1
fi

# Build only once
echo "Building chain with features: cargo build -p centrifuge-chain --release --features runtime-benchmarks"
cargo build -p centrifuge-chain --release --features runtime-benchmarks
check $?

weight_path="${runtime_path}/src/weights"
# Create a tmp director
build_path="/tmp/${weight_path}"
mkdir -p "${build_path}"
check $?

# Create mod.rs file
touch "${build_path}/mod.rs"
check $?

cat license-template-gplv3.txt >> "${build_path}/mod.rs"
check $?

# Collect all possible benchmarks the respective runtime provides
all_pallets=$(
  ./target/release/centrifuge-chain benchmark pallet --list --chain="${chain}" | tail -n+2 | cut -d',' -f1 | sort | uniq
)
for pallet in $all_pallets
do
    output="${build_path}/${pallet}.rs"

    if [[ $pallet != "frame_system" ]]; then
      run_benchmark $pallet $output
      check $?

      echo "pub mod ${pallet};" >> "${build_path}/mod.rs"
      check $?
    else
      echo "WARNING: Skipping frame_system. Please re-enable at Polkadot v1.0.0+ support."
    fi
done

echo "Removing old weights in '${weight_path}'"
rm -r ${weight_path}
check $?

echo "Moving new weights from '${build_path}' into '${weight_path}'"
mv -f ${build_path} ${weight_path}

# Run cargo fmt to ensure that all the new weight.rs files are properly formatted.
cargo fmt
check $?

echo "Cleaning up artifacts in '/tmp/${runtime_path}'"
rm -r "/tmp/${runtime_path}"

echo "Benchmarking finished."
