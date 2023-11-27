#!/bin/bash
if [ -d "/data/relay-chain" ]
then
    echo "Detected relay-chain folder. Renaming to polkadot..."
    mv /data/relay-chain /data/polkadot
fi

centrifuge-chain "$@"