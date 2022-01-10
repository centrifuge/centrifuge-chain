#!/usr/bin/env bash

set -e

if [ "$#" -ne 1 ]; then
	echo "Please provide the number of initial collators!"
	exit 1
fi

NETWORK=${NETWORK:-centrifuge}

generate_secret() {
  subkey generate -n $NETWORK | grep "Secret seed" | awk '{ print $3 }'
}

generate_account_id() {
	subkey inspect ${3:-} ${4:-} "$SECRET//$1//$2" | grep "Account ID" | awk '{ print $3 }'
}

generate_address() {
	subkey inspect ${3:-} ${4:-} "$SECRET//$1//$2" | grep "SS58 Address" | awk '{ print $3 }'
}

generate_public_key() {
	subkey inspect ${3:-} ${4:-} "$SECRET//$1//$2" | grep "Public" | awk '{ print $4 }'
}

generate_address_and_public_key() {
	ADDRESS=$(generate_address $1 $2 $3)
	PUBLIC_KEY=$(generate_public_key $1 $2 $3)

	printf "//$ADDRESS\nhex![\"${PUBLIC_KEY#'0x'}\"].unchecked_into(),"
}

generate_address_and_account_id() {
	ACCOUNT=$(generate_account_id $1 $2 $3)
	ADDRESS=$(generate_address $1 $2 $3)
	if ${4:-false}; then
		INTO="unchecked_into"
	else
		INTO="into"
	fi

	printf "//$ADDRESS\nhex![\"${ACCOUNT#'0x'}\"].$INTO(),"
}

V_NUM=$1

SECRET=$(generate_secret)
printf "Secret Seed: $SECRET\n\n"

NODE_KEYS=""
SESSION_KEYS=""
AUTHORITIES=""
for i in $(seq 1 $V_NUM); do
  AUTHORITIES+="(\n"
  AUTHORITIES+="$(generate_address_and_account_id $i stash '-n'$NETWORK )\n"
  AUTHORITIES+="$(generate_address_and_account_id $i aura '-n'$NETWORK true )\n"
  AUTHORITIES+="),\n"
  NODE_KEYS+="NodeKey $i: $(subkey generate-node-key)\n"
  SESSION_KEYS+="AuraKey $i: $SECRET//$i//aura\n"
done

printf "$AUTHORITIES"

printf "DEVOPS SECTION\n"
printf "$NODE_KEYS"
printf "$SESSION_KEYS"


