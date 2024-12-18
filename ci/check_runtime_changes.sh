#!/bin/bash

# 
# - if runtime files are changed:
#	- Check if the spec_version is incremented
# 	- If spec_version incremented, ensure impl_version is 0 and exit successfully
# 	- if spec_version is NOT incremented, check if the impl_version is incremented
# 		- if impl_version incremented, then exit successfully
# 		- if impl_version is NOT incremented then fail the script
# - if runtime files are NOT changed, exit successfully
#

# origin/master as the base commit
BASE_COMMIT="origin/master"
PR_COMMIT="HEAD"
VERSIONS_FILE="runtime/src/lib.rs"

echo "PR commit: ${PR_COMMIT}"
echo "BASE commit: ${BASE_COMMIT}"

PR_BRANCH=${TRAVIS_PULL_REQUEST_BRANCH}

echo "PR branch ${PR_BRANCH}"

# use color in echo for indicating success or fail
red='\033[0;31m'
bold='\033[1m'
nc='\033[0m' # No Color
green='\033[01;32m'
yellow='\033[01;33m'

OK="${green}${block}OK${nc}"
ERROR="${red}${block}ERROR${nc}"
FATAL="${red}${block}FATAL${nc}"


# show the diff of origin/master and this PR sha
CHANGED_FILES=$(git diff --name-only ${BASE_COMMIT} ${PR_COMMIT} 2>&1 )
GIT_STATUS=$?
if (( $GIT_STATUS != 0 ))
then
	echo -e "${red}${bold}GIT ERROR${nc}: $CHANGED_FILES"
	exit 1
fi

# count the number of files changed in runtime directory
RUNTIME_FILE_CHANGED=$(echo "${CHANGED_FILES}" | grep -e ^runtime/ | wc -l)

echo "There are ${RUNTIME_FILE_CHANGED} file(s) changed in runtime "

# If there are no changes in the runtime file, exit successfully
if (( $RUNTIME_FILE_CHANGED == 0 ))
then
	echo -e "| ${OK} Nothing is changed in runtime"
	exit 0
fi

# Extract the spec_version  and impl_version from origin/master and the PR
BASE_SPEC_VERSION=$(git show ${BASE_COMMIT}:${VERSIONS_FILE} | sed -n -r "s/^[[:space:]]+spec_version: +([0-9]+),$/\1/p")
BASE_IMPL_VERSION=$(git show ${BASE_COMMIT}:${VERSIONS_FILE} | sed -n -r "s/^[[:space:]]+impl_version: +([0-9]+),$/\1/p")

PR_SPEC_VERSION=$(git show ${PR_COMMIT}:${VERSIONS_FILE} | sed -n -r "s/^[[:space:]]+spec_version: +([0-9]+),$/\1/p")
PR_IMPL_VERSION=$(git show ${PR_COMMIT}:${VERSIONS_FILE} | sed -n -r "s/^[[:space:]]+impl_version: +([0-9]+),$/\1/p")

# Show the changes in the versions
echo "|"
echo "| ${BASE_COMMIT} spec_version: ${BASE_SPEC_VERSION}"
echo "| ${BASE_COMMIT} impl_version: ${BASE_IMPL_VERSION}"
echo "| "
echo "| PR spec_version: ${PR_SPEC_VERSION}"
echo "| PR impl_version: ${PR_IMPL_VERSION}"
echo "|"

# spec_version should never decremented, show a fatal alert it that's the case
if (( $PR_SPEC_VERSION < $BASE_SPEC_VERSION ))
then
	echo -e "${FATAL}: ${yellow}spec_version${nc} is decremented"
	exit 1
fi

# Check if the PR spec version is incremented
if (( $PR_SPEC_VERSION > $BASE_SPEC_VERSION ))
then
	echo -e "| spec_version is changed: ${BASE_SPEC_VERSION} -> ${PR_SPEC_VERSION}"
	# Ensure impl_version in the PR is set to 0 when spec_version is incremented
	if (( $PR_IMPL_VERSION == 0 ))
	then
		echo -e "| ${OK}: ${yellow}impl_version${nc} is set to 0"
		exit 0
	else
		# The impl_version is not reset to 0 and should fail
		echo -e "| ${ERROR}: ${yellow}impl_version${nc} must be reset to 0 when ${yellow}spec_version${nc} is incremented"
		exit 1
	fi
# The spec_version is not changed
else
	# if spec_version is not incremented
	# Check if impl_version is incremented
	if (( $PR_IMPL_VERSION > $BASE_IMPL_VERSION ))
	then
		echo -e "| impl_version is changed: ${BASE_IMPL_VERSION} -> ${PR_IMPL_VERSION}"
		echo -e "| ${OK}: ${yellow}impl_version${nc} is incremented"
		exit 0
	else
		# The impl_version is not incremented, and should fail
		echo -e "| ${ERROR}: ${yellow}impl_version${nc} is NOT incremented"
		echo -e "|	Note: either ${yellow}impl_version${nc} or ${yellow}spec_version${nc} should be incremented when there is changes in the runtime"
		exit 1
	fi
fi

exit 1
