#!/bin/bash
if [ "$1" == "--help" ]; then
    echo "No arguments detected, printing help and exiting..."
    centrifuge-chain "$@"
    exit 0
fi

# Fix to account for Polkadot's renaming of their DB folder from
# relay-chain to polkadot. Probably not needed after all nodes are upgraded
# beyond Polkadot 0.9.42+
BASE_PATH=""
for ARG in "$@"
do
    if [[ $ARG == --base-path=* ]]; then
        BASE_PATH="${ARG#*=}"
        break
    fi
done
if [ -z "$BASE_PATH" ]
then
    BASE_PATH="/data"
fi

if [ -d "${BASE_PATH}/relay-chain" ]
then
    relay_chain_size=$(du -s "${BASE_PATH}/relay-chain" | cut -f1)

    echo "Detected relay-chain folder. Renaming to polkadot..."
    if [ -d "${BASE_PATH}/polkadot" ]
    then
        if [ -d "${BASE_PATH}/polkadot" ]
        then
            polkadot_size=$(du -s "${BASE_PATH}/polkadot" | cut -f1)
            if [ "$polkadot_size" -ge "$relay_chain_size" ]
            then
                echo -e "\e[1;31m${BASE_PATH}/polkadot\e[0m folder is larger than or equal to \e[1;31m${BASE_PATH}/relay-chain\e[0m"
                echo "This is unexpected. Manual check required."
                echo "HINT: Delete one of the two folders to preserve that DB"
                exit 1
            else
                echo "${BASE_PATH}/polkadot is smaller than ${BASE_PATH}/relay-chain"
                echo "Creating backup of ${BASE_PATH}/polkadot before replacing it..."
                mv "${BASE_PATH}/polkadot" "${BASE_PATH}/polkadot.bak"
                rm -rf "${BASE_PATH}/polkadot"            
            fi
        fi    
    fi
    mv -f "${BASE_PATH}/relay-chain" "${BASE_PATH}/polkadot"
fi

# Start the chain
centrifuge-chain "$@"