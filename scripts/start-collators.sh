function cleanup() {
    for (( i=0; i<${#pids[@]}; i++)); {
        echo "killing process ${pids[i]}"
        kill ${pids[i]}
    }
    echo ""
    echo "Stopped the collators. Goodbye"
    exit
}

### Config
# Parachain id of collators
parachain_id=10001
# Paths to chain spec files
relay_config=../../cent-polka/polkadot/rococo-chachacha-local-cfde-real-overseer.json
#relay_config=../../polkadot/rococo-local-custom-relay.json
collator_config=charcoal-chachacha-local
# Output files for collator processes
nodes_per_parachain=2
num_parachains=1

### Internal config
# All collators point to this relay-chain node
relay_bootnode=/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWAf7iThMCPgkAncjTfiqPb2Q6nBakwY92shW347ucVbXB
# Ports start at these values and increment on each new collator
collator_port=40335
relay_port=30335
ws_port=9946

### Don't change
root_port=$collator_port
# Track pid for cleanup
pids=()
# P2p keys taken from node output (for now)
## This hack limits the num of parachains to pre-made node key files
p2p_keys=(12D3KooWABE1PTn4S1ZRohkpDm4hSjYc5gxefUxn6ySf4z6KF96E
          12D3KooWFU1guJYkY9B5mFRYuueWBfyfd7ksRoTM2dg6zJhQSFXE)

# Start all collators in parallel
for (( p=1; p<=num_parachains; p++)); {
    for (( n=1; n<=nodes_per_parachain; n++)); {
        # Output log can be read here
        col_output_path=/tmp/col_${parachain_id}_$n.log

        # Start a collator
        if [ $n -eq 1 ]
        then
            # Set root port for fixed collator of parachain
            root_port=$collator_port

            # First collator of a parachain set will be fixed to a key
            ../target/release/centrifuge-chain \
                --collator \
                --tmp \
                --parachain-id $parachain_id \
                --chain "$collator_config" \
                --port $collator_port \
                --node-key-file "collator${p}.key" \
                --ws-port $ws_port \
                -- \
                --execution wasm \
                --chain $relay_config \
                --port $relay_port \
                --bootnodes $relay_bootnode \
                    &> $col_output_path &
        else
            # Other collators will point to first as a bootnode
            ../target/release/centrifuge-chain \
                --collator \
                --tmp \
                --parachain-id $parachain_id \
                --port $collator_port \
                --chain "$collator_config" \
                --bootnodes /ip4/127.0.0.1/tcp/$root_port/p2p/${p2p_keys[p-1]} \
                --ws-port $ws_port \
                -- \
                --execution wasm \
                --chain $relay_config \
                --port $relay_port \
                --bootnodes $relay_bootnode \
                    &> $col_output_path &
        fi

        # Add pid to list
        #echo "adding $! to pids"
        pids+=($!)
        echo "collator $n is running - output at $col_output_path..."

        # Increment ports
        collator_port=$((collator_port+1))
        ws_port=$((ws_port+1))
        relay_port=$((relay_port+1))
    }

    # New parachain
    parachain_id=$((parachain_id+1))
}

# Kill processes on ctrl-c
trap cleanup SIGINT

# UI control loop
while true
do
    sleep 100
done
