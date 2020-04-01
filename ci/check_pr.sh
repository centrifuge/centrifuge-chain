#!/bin/bash
#
#set -x
# check for any changes in the runtime/ . if
# there are any changes found, it should mark the PR breaksconsensus and
# "auto-fail" the PR if there isn't a change in the runtime/src/lib.rs file
# that alters the version.

# get the commit sha for this PR
CI_COMMIT_SHA=$(git rev-parse HEAD)
echo "commit: ${CI_COMMIT_SHA}"

LABEL_BREAKS_API="breaks-api"
LABEL_CHANGES_RUNTIME="changes-runtime"
red='\033[0;31m'
bold='\033[1m'
nc='\033[0m' # No Color
green='\033[01;32m'
yellow='\033[01;33m'


boldprint () { printf "|\n| \033[1m${@}\033[0m\n|\n" ; }

boldprint "latest 10 commits of ${CI_COMMIT_REF_NAME}"
git log --graph --oneline --decorate=short -n 10

# base the origin/master as the base commit
BASE_COMMIT="origin/master"
VERSIONS_FILE="runtime/src/lib.rs"

# show the diff of origin/master and this PR sha
CHANGED_FILES=$(git diff --name-only ${BASE_COMMIT}...${CI_COMMIT_SHA})


# count the number of files changed in runtime directory
RUNTIME_FILE_CHANGED=$(echo "${CHANGED_FILES}" | grep -e ^runtime/ | wc -l)




# Get the PR id from the commit hash using github API:
# ie: curl -s https://api.github.com/search/issues?q=sha:22681fb3ae899448d73124b047f006ce84164234
#
# Use jq to extract the PR number 
#	jq '.number'
#   
# The PR number is used to get the PR information
# ie: https://api.github.com/repos/centrifuge/centrifuge-chain/pulls/119
# The labels is then extracted from the PR info using
#   jq '.labels' | jq ' .[] | .name '
#
# ie: curl -s https://api.github.com/repos/centrifuge/centrifuge-chain/pulls/119 | jq '.labels' | jq -r '.[] | .name'
# This will list down the list of labels on that PR




# returns the PR number where a commit hash belongs to
github_pr_from_commit () {
	commit_details=$(curl -s https://api.github.com/search/issues?q=sha:${1})
	first_result=$(echo "${commit_details}" | jq '.items[0]')
	pr_id=$(echo "${first_result}" | jq '.number' )
	echo "${pr_id}"
}



# returns the label names separated by a new line
# Note: the label names is double quoted
github_label_from_pr () {
	pr_info=$(curl -s https://api.github.com/repos/centrifuge/centrifuge-chain/pulls/${1})
	labels=$(echo "${pr_info}" | jq '.labels' )
	if [ "$labels" != "null" ]; then 
		label_names=$(echo "${labels}" | jq -r '.[] | .name')
		echo "${label_names}"
	fi
}


PR_ID=$(github_pr_from_commit "${CI_COMMIT_SHA}")
PR_LABEL=$(github_label_from_pr "${PR_ID}")


echo -e ""
echo -e "| PR labels: ${yellow}${PR_LABEL}${nc}"


LABEL_BREAK_API_COUNT=$(echo "${PR_LABEL}" | grep -w ${LABEL_BREAKS_API} | wc -l)
LABEL_CHANGES_RUNTIME_COUNT=$(echo "${PR_LABEL}" | grep -w ${LABEL_CHANGES_RUNTIME} | wc -l)


if [ $RUNTIME_FILE_CHANGED != "0" ]
then
	echo "There are ${RUNTIME_FILE_CHANGED} file(s) changed in runtime "

	if [ "${LABEL_BREAK_API_COUNT}" != "0" ]
	then
		add_spec_version="$(git diff ${BASE_COMMIT}...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r "s/^\+[[:space:]]+spec_version: +([0-9]+),$/\1/p")"
		sub_spec_version="$(git diff ${BASE_COMMIT}...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r "s/^\-[[:space:]]+spec_version: +([0-9]+),$/\1/p")"

		if [ "${add_spec_version}" != "${sub_spec_version}" ]
		then
			echo -e "| ${green}${bold}OK:${nc} PR has label ${yellow}${LABEL_BREAKS_API}${nc} and spec_version is changed.. "
			exit 0
		else
			echo -e "| ${red}${bold}ERROR:${nc} PR has label ${yellow}${LABEL_BREAKS_API}${nc}, but spec_version remains the same in ${VERSIONS_FILE}"
		fi
	elif [ "${LABEL_CHANGES_RUNTIME_COUNT}" != "0" ]
	then

		add_impl_version="$(git diff ${BASE_COMMIT}...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r 's/^\+[[:space:]]+impl_version: +([0-9]+),$/\1/p')"
		sub_impl_version="$(git diff ${BASE_COMMIT}...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r 's/^\-[[:space:]]+impl_version: +([0-9]+),$/\1/p')"

		if [ "${add_impl_version}" != "${sub_impl_version}" ]
		then
			echo -e "| ${green}${bold}OK:${nc} PR has label ${yellow}${LABEL_CHANGES_RUNTIME}${nc} and impl_version is changed..."
			exit 0
		else
			echo -e "| ${red}${bold}ERROR:${nc} PR has label ${yellow}${LABEL_CHANGES_RUNTIME}${nc}, but impl_version remains the same in ${VERSIONS_FILE}"
		fi
	else
		echo -e ""
		echo -e "| ${red}${bold}ERROR:${nc} There are changes in runtime but PR has no required label"
		echo -e "|    Required label is one of: ${yellow}${LABEL_BREAKS_API}${nc},${yellow}${LABEL_CHANGES_RUNTIME}${nc}"
		if [ "${PR_LABEL}" != "" ]
		then
			echo -e "| PR has these labels: ${yellow}${PR_LABEL}${nc}"
		fi
		echo -e "| "
	fi
else
	echo "OK: No changes in runtime, no need for version change"
	exit 0
fi

exit 1
