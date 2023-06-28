set -eu

runtime=$1

run_benchmark() {
  pallet=$1

  cmd="target/release/centrifuge-chain benchmark pallet \
    --chain="${chain}" \
    --steps=2 \
    --repeat=1 \
    --pallet="${pallet}" \
    --extrinsic=* \
    --execution=wasm \
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

for pallet in $all_pallets
do
    run_benchmark $pallet
done
