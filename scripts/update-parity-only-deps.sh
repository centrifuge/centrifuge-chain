#!/usr/bin/env bash

declare -a deps=("https://github.com/paritytech/substrate" "https://github.com/paritytech/cumulus" "https://github.com/paritytech/polkadot" "https://github.com/open-web3-stack/open-runtime-module-library" "https://github.com/centrifuge/chainbridge-substrate" "https://github.com/centrifuge/unique-assets" )
declare -a parity_deps=()

for dep in ${deps[@]}; do
   echo $dep
   tomls=$(find ./ -type f -iname "*.toml" -exec grep -l "${dep}" {} \;)
   for tml in ${tomls[@]}; do
      inner_deps=$(cat $tml | grep "${dep}" | awk '{print $1}')
      for indep in ${inner_deps}; do
        if [[ $indep = "grandpa-primitives" ]]
        then
          parity_deps+=("sp-finality-grandpa")
        else
          parity_deps+=($indep)
        fi
      done
   done
done

uniq_deps=($(for v in "${parity_deps[@]}"; do echo "$v";done| sort| uniq| xargs))

update_params=""
for value in "${uniq_deps[@]}"
do
     update_params+=" -p $value"
done

echo "${update_params}"

cargo update $update_params

