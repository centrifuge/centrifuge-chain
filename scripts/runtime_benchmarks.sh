#!/usr/bin/env bash

set -u

runtime=$1

check() {
  return_code=$1

  if [[ $return_code -ne 0 ]]
  then
    echo "Error: Intermediate error occured. Cleaning up artifacts in '/tmp/${runtime_path}'"
    rm -r "/tmp/${runtime_path}"
    echo "Aborting!"
    exit 1
  fi
}

run_benchmark() {
  pallet=$1
  output=$2

  cmd="target/release/centrifuge-chain benchmark \
    --chain="${chain}" \
    --steps=50 \
    --repeat=20 \
    --pallet="${pallet}" \
    --extrinsic=* \
    --execution=wasm \
    --wasm-execution=compiled \
    --heap-pages=4096 \
    --output="${output}" \
    --template=./scripts/runtime-weight-template.hbs"

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
echo "Building chain with features: cargo build --release --features runtime-benchmarks"
cargo build --release --features runtime-benchmarks
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
all_pallets=$(sed -n -e '/add_benchmark!/p' "${runtime_path}/src/lib.rs" | tr -d '[:space:]' | tr -d '('| tr -d ')' | tr ';' ' ')
for pallet in $all_pallets
do
    # Trim string into the final array
    pallet=${pallet//"add_benchmark!"/}
    IFS=', ' read -r -a array <<< $(echo $pallet | tr ',' ' ')

    pallet=${array[2]}
    output="${build_path}/${array[2]}.rs"

    run_benchmark $pallet $output
    check $?

    echo "pub mod ${array[2]};" >> "${build_path}/mod.rs"
    check $?
done

echo "Removing old weights in '${weight_path}'"
rm -r ${weight_path}
check $?

echo "Moving new weights from '${build_path}' into '${weight_path}'"
mv -f ${build_path} ${weight_path}

# since benchmark generates a weight.rs file that may or may not cargo fmt'ed.
# so do cargo fmt here.
cargo fmt
check $?

echo "Cleaning up artifacts in '/tmp/${runtime_path}'"
rm -r "/tmp/${runtime_path}"

echo "Benchmarking finished."