RUNTIME=$1
FEATURES=$3

# usage: ./scripts/check_benchmarks.sh <environment> <debug/release> [--features==X,Y,Z]

if [[ "$2" == "release" ]]; then
    MODE="release"
    CARGO_MODE="--release"
else
    MODE="debug"
    CARGO_MODE=""
fi

if [[ -z "$3" ]]; then
    FEATURES="--features=runtime-benchmarks"
fi

run_benchmark() {
  pallet=$1

  cmd="target/${mode}/centrifuge-chain benchmark pallet \
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

if [[ $RUNTIME == "development" ]];
then
  chain="development-local"
elif [[ $RUNTIME == "centrifuge" ]];
then
  chain="centrifuge-dev"
elif [[ $RUNTIME == "altair" ]];
then
  chain="altair-dev"
else
  echo "Unknown runtime. Aborting!"
  exit 1;
fi

echo Running in ${MODE} with ${FEATURES}

cargo build $CARGO_MODE $FEATURES

all_pallets=$(
  ./target/${MODE}/centrifuge-chain benchmark pallet --list --chain="${chain}" | tail -n+2 | cut -d',' -f1 | sort | uniq
)

for pallet in $all_pallets
do
    run_benchmark $pallet
done
