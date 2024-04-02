set -eu

runtime=$1
pallet_input=${2:-}

run_benchmark() {
  pallet=$1

  cmd="target/release/centrifuge-chain benchmark pallet \
    --chain="${chain}" \
    --steps=2 \
    --repeat=1 \
    --pallet="${pallet}" \
    --extrinsic=* \
    --wasm-execution=compiled \
    --heap-pages=4096"

    echo "Running benchmark for pallet '${pallet}'"
    echo "${cmd}"
    ${cmd}
}

if [[ $runtime == "development" ]];
then
  chain="development-local"
elif [[ $runtime == "centrifuge" ]];
then
  chain="centrifuge-dev"
elif [[ $runtime == "altair" ]];
then
  chain="altair-dev"
else
  echo "Unknown runtime. Aborting!"
  exit 1;
fi

cargo build -p centrifuge-chain --release --features runtime-benchmarks

all_pallets=$(
  ./target/release/centrifuge-chain benchmark pallet --list --chain="${chain}" | tail -n+2 | cut -d',' -f1 | sort | uniq
)

if [ -n "$pallet_input" ];
then
  echo "Only benchmarking a single pallet: $pallet_input"
  run_benchmark $pallet_input
else
  echo "Benchmarking all pallets"
  for pallet in $all_pallets
  do
      if [[ $pallet != "frame_system" ]]; then
        run_benchmark $pallet
      else
        echo "WARNING: Skipping frame_system. Please re-enable at Polkadot v1.0.0+ support."
      fi
  done
fi


