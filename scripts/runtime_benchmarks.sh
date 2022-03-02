#!/usr/bin/env bash

runtime=$1

run_benchmark() {
  pallet=$1
  output=$2

  cargo run --release --features runtime-benchmarks -- benchmark \
    --chain="${runtime}" \
    --steps=50 \
    --repeat=20 \
    --pallet="${pallet}" \
    --extrinsic=* \
    --execution=wasm \
    --wasm-execution=compiled \
    --heap-pages=4096 \
    --output="${output}" \
    --template=./scripts/frame-weight-template.hbs
}

# non_blocking_wait() {
#     PID=$1
#     if [ ! -d "/proc/$PID" ]; then
#         wait $PID
#         CODE=$?
#     else
#         CODE=127
#     fi
#     return $CODE
# }

echo "Benchmarking pallets for runtime ${runtime}..."

# Find the correct path to create weights in
if [[ $runtime == "development" ]];
then
  runtime_path="runtime/development"
elif [[ $runtime == "centrifuge" ]];
then
  runtime_path="runtime/centrifuge"
elif [[ $runtime == "altair" ]];
then
  runtime_path="runtime/altair"
else
  echo "Unknown runtime. Aborting!"
  exit 1;
fi

# Create director if not already there
# We do not care if it already exists.
weight_path="${runtime_path}/src/weights"
mkdir "${weight_path}" 2> /dev/null

# # PIDs of started benchmarks
# pids=()

# Collect all possible benchmarks the respective runtime provides
all_pallets=$(sed -n -e '/add_benchmark!/p' "${runtime_path}/src/lib.rs" | tr -d '[:space:]' | tr -d '('| tr -d ')' | tr ';' ' ')
for pallet in $all_pallets
do
    # Trim string into the final array
    pallet=${pallet//"add_benchmark!"/}
    IFS=', ' read -r -a array <<< $(echo $pallet | tr ',' ' ')

    pallet=${array[2]}
    output="${weight_path}/${array[2]}.rs"

    run_benchmark $pallet $output
    # run_benchmark $pallet $output &
    # pids+=($!)
done

# num_pids=$(expr ${#pids[@]} - 1)
# x=0
# while [[ $x -le $num_pids ]]; do
#    for pid in "${pids[@]}"
#    do
#      echo "$pid"
#      non_blocking_wait $pid
#      CODE=$?
#      if [ $CODE -ne 127 ]; then
#          x+=1
#      fi
#    done
#    # Wait some time before checking again
#    sleep 2
# done


# since benchmark generates a weight.rs file that may or may not cargo fmt'ed.
# so do cargo fmt here.
cargo fmt

echo "Benchmarking finished."