#!/usr/bin/env bash

# Usage
# ./scripts/tests.sh check|test "-F=feature1,feature2,..." <--no-run>

all_crates=$(
    cargo workspaces list
)

cargo_action() {
    action=$1
    package=$2
    features=$3
    norun=$4

    echo -e "$testing_prompt cargo $action -p $package $features"
    cargo $action -p $package $features $norun

    if [[ $? -ne 0 ]]; then
        echo "Aborting!"
        exit 1
    fi
}

ESC="\033"
testing_prompt="$ESC[1;36m     Testing$ESC[0m"

cargo workspaces list > /dev/null
if [[ $? -ne 0 ]]; then
    echo try: \'cargo install cargo-workspaces\' before using this crate
fi

for crate in $all_crates
do
    if [[ $2 == *"runtime-benchmarks"* || $2 == *"try-runtime"* ]]; then
        if [[ "$crate" == "proofs" ]]; then
            echo "Skipping!"
            continue
        fi

        if [[ "$crate" == "runtime-integration-tests" ]]; then
            echo "Skipping!"
            continue
        fi
    fi

    cargo_action $1 $crate $2 $3
done
