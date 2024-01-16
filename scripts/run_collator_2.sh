#!/usr/bin/env bash

# this script runs the centrifuge-chain after fetching
# appropriate bootnode IDs
#
# this is _not_ a general-purpose script; it is closely tied to the
# root docker-compose.yml

set -e -o pipefail

ctpc="./target/release/centrifuge-chain"

if [ ! -x "$ctpc" ]; then
    echo "FATAL: $ctpc does not exist or is not executable"
    exit 1
fi

# name the variable with the incoming args so it isn't overwritten later by function calls
args=( "$@" )

alice="127.0.0.1"
bob="127.0.0.1"
alice_p2p_port="30333"
alice_rpc_port="9944"
bob_p2p_port="30344"
bob_rpc_port="9945"
chain="${RELAY_CHAIN_SPEC:-./node/res/rococo-local.json}"


get_id () {
    node="$1"
    port="$2"
    curl -sS \
        -H 'Content-Type: application/json' \
        --data '{"id":1,"jsonrpc":"2.0","method":"system_localPeerId"}' \
        "http://$node:$port" |\
    jq -r '.result'

}

bootnode () {
    node="$1"
    p2p_port="$2"
    rpc_port="$3"
    id=$(get_id "$node" "$rpc_port")
    if [ -z "$id" ]; then
        echo >&2 "failed to get id for $node"
        exit 1
    fi
    echo "/ip4/$node/tcp/$p2p_port/p2p/$id"
}

args+=( "--" "--wasm-execution=compiled" "--execution=wasm" "--chain=${chain}" "--bootnodes=/ip4/127.0.0.1/tcp/30334/p2p/12D3KooWBnnrKEDxqk9v9ZsMJwzXbMJvAhn4k2xZGJDBUwH3qk87" )

set -x
"$ctpc" "${args[@]}"
