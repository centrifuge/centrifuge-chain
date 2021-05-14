#!/usr/bin/env bash

set -e

cargo build --release
rm -rf /tmp/centrifuge-chain

yarn global add @polkadot/api-cli@0.32.1

genesis=$(./target/release/centrifuge-chain export-genesis-state --chain=charcoal-chachacha-local)
wasm="0x"$(xxd -p ./target/release/wbuild/centrifuge-chain-runtime/centrifuge_chain_runtime.compact.wasm)
wasm=$(echo $wasm | sed "s/ //g")
#echo $genesis > /tmp/gen.txt
#echo $wasm > /tmp/wasm.txt

#polkadot-js-api \
#        --ws ws://0.0.0.0:9944 \
#        --seed "//Alice" \
#        --sudo \
#        tx.parasSudoWrapper.sudoScheduleParaInitialize \
#        2000 \
#        "{ \"genesisHead\":\"${genesis?}\", \"validationCode\": \"${wasm}\", \"parachain\": true }"


# run collator
./test/scripts/run_collator.sh \
  --chain=charcoal-chachacha-local --alice \
  --base-path=/tmp/centrifuge-chain/data \
  --port 30355 \
  --rpc-port 9936 \
  --ws-port 9946 \
  --rpc-external \
  --rpc-cors all \
  --ws-external \
  --rpc-methods=Unsafe \
  --log="main,info" \
