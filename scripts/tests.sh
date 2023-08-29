#!/usr/bin/env bash

# NOTE
# If some crate fails, you can continue the testing after fixing it as follows:
# ./scripts/tests.sh <failed-crate>

start_from=$1

all_crates=$(
    cargo workspaces list
)

cargo_action() {
    action=$1
    package=$2
    features=$3

    echo -e "$testing_prompt cargo $action -p $package $features"
    cargo $action -p $package $features

    if [[ $1 -ne 0 ]]; then
        echo "Aborting!"
        exit 1
    fi
}

ESC="\033"
testing_prompt="$ESC[1;36m     Testing$ESC[0m"
go=0

if [[ -z "$start_from" ]]; then
    go=1
fi

cargo workspaces list > /dev/null
if [[ $1 -ne 0 ]]; then
    echo try: \'cargo install cargo-workspaces\' before using this crate
fi


for crate in $all_crates
do
    if [[ "$start_from" == "$crate" ]]; then
        go=1
    fi

    if [[ $go -eq 0 ]]; then
        # Skipping until found a crate as starting point
        continue
    fi

    cargo_action check $crate
    cargo_action test $crate --no-run

    if [[ "$crate" == "proofs" ]]; then
        # proofs does not have either try-runtime or runtime-benchmarks features
        continue
    fi

    cargo_action check $crate "-F runtime-benchmarks"
    cargo_action test $crate "-F runtime-benchmarks" --no-run

    if [[ "$crate" == "runtime-integration-tests" ]]; then
        # runtime-integration-test does not have try-runtime feature
        continue
    fi

    cargo_action check $crate "-F try-runtime"
    cargo_action test $crate "-F try-runtime" --no-run

    cargo_action check $crate "-F runtime-benchmarks,try-runtime"
    cargo_action test $crate "-F runtime-benchmarks,try-runtime" --no-run
done

# Run all tests all
cargo test --workspace -F runtime-benchmarks,try-runtime
