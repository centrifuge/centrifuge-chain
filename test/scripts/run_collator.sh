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

alice="0.0.0.0"
bob="0.0.0.0"
alice_p2p_port="30333"
alice_rpc_port="9933"
bob_p2p_port="30344"
bob_rpc_port="9935"


get_id () {
    node="$1"
    port="$2"
    curl -sS \
        -H 'Content-Type: application/json' \
        --data '{"id":1,"jsonrpc":"2.0","method":"system_localPeerId"}' \
        "$node:$port" |\
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

args+=( "--" "--bootnodes=$(bootnode "$alice" "$alice_p2p_port" "$alice_rpc_port")" "--bootnodes=$(bootnode "$bob" "$bob_p2p_port" "$bob_rpc_port")" )

set -x
"$ctpc" "${args[@]}"
