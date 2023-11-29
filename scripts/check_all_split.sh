#!/usr/bin/env bash

# Usage
# ./scripts/tests.sh <"-F=feature1,feature2,...">

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

# Checking cargo check
for crate in $all_crates
do
    if [[ $1 == *"runtime-benchmarks"* || $1 == *"try-runtime"* ]]; then
        if [[ "$crate" == "proofs" ]]; then
            echo "Skipping!"
            continue
        fi

        if [[ "$crate" == "runtime-integration-tests" ]]; then
            echo "Skipping!"
            continue
        fi
    fi

    cargo_action check $crate $1
done

# Checking cargo test
for crate in $all_crates
do
    if [[ $1 == *"runtime-benchmarks"* || $1 == *"try-runtime"* ]]; then
        if [[ "$crate" == "proofs" ]]; then
            echo "Skipping!"
            continue
        fi

        if [[ "$crate" == "runtime-integration-tests" ]]; then
            echo "Skipping!"
            continue
        fi
    fi

    cargo_action test $crate $1 --no-run
done
