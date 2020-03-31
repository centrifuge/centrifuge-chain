#!/bin/sh
#

# check for any changes in the runtime/ . if
# there are any changes found, it should mark the PR breaksconsensus and
# "auto-fail" the PR if there isn't a change in the runtime/src/lib.rs file
# that alters the version.

# get the commit sha for this PR
CI_COMMIT_SHA=$(git rev-parse HEAD)


boldprint () { printf "|\n| \033[1m${@}\033[0m\n|\n" ; }

boldprint "latest 10 commits of ${CI_COMMIT_REF_NAME}"
git log --graph --oneline --decorate=short -n 10

# base the origin/master as the base commit
BASE_COMMIT="origin/master"
VERSIONS_FILE="runtime/src/lib.rs"

# show the diff of origin/master and this PR sha
CHANGED_FILES=$(git diff --name-only ${BASE_COMMIT}...${CI_COMMIT_SHA})

echo "Changed files $CHANGED_FILES"

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
# ie: curl -s https://api.github.com/repos/centrifuge/centrifuge-chain/pulls/119 | jq '.labels' | jq '.[] | .name'
# This will list down the list of labels on that PR




# returns the PR number where a commit hash belongs to
github_pr_from_commit () {
	commit_details=$(curl -s https://api.github.com/search/issues?q=sha:${1})
	first_result=$(echo ${commit_details} | jq '.items[0]')
	pr_id=$(echo ${first_result} | jq '.number' )
	echo ${pr_id}
}



# returns the label names separated by a new line
# Note: the label names is double quoted
github_label_from_pr () {
	pr_info=$(curl -s https://api.github.com/repos/centrifuge/centrifuge-chain/pulls/${1})
	labels=$(echo ${pr_info} | jq '.labels' )
	if [ "$labels" != "null" ]; then 
		label_names=$(echo ${labels} | jq '.[] | .name')
		echo ${label_names}
	fi
}


PR_ID=$(github_pr_from_commit ${CI_COMMIT_SHA})
pr_label=$(github_label_from_pr "${PR_ID}")

echo "pr_label:[${pr_label}]"

LABEL_MARKER="breakapi"

LABELED_MARKER_COUNT=$(echo -e "${pr_label}" | grep -w ${LABEL_MARKER} | wc -l)

if [ $RUNTIME_FILE_CHANGED != "0" ]
	then
		echo "There are ${RUNTIME_FILE_CHANGED} files changed in runtime "

	if [ "${LABELED_MARKER_COUNT}" != "0" ]
	then
		add_spec_version="$(git diff ${BASE_COMMIT}...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r "s/^\+[[:space:]]+spec_version: +([0-9]+),$/\1/p")"
		sub_spec_version="$(git diff ${BASE_COMMIT}...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r "s/^\-[[:space:]]+spec_version: +([0-9]+),$/\1/p")"

		if [ "${add_spec_version}" != "${sub_spec_version}" ]
		then
			echo "OK: spec_version is changed.. "
			exit 0
		else
			echo "ERROR: spec_version should be changed in ${VERSIONS_FILE}"
		fi
	else
		echo "Not a breaking change"

		add_impl_version="$(git diff tags/release...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r 's/^\+[[:space:]]+impl_version: +([0-9]+),$/\1/p')"
		sub_impl_version="$(git diff tags/release...${CI_COMMIT_SHA} ${VERSIONS_FILE} \
			| sed -n -r 's/^\-[[:space:]]+impl_version: +([0-9]+),$/\1/p')"

		if [ "${add_impl_version}" != "${sub_impl_version}" ]
		then
			echo "OK: impl_version is changed..."
			exit 0
		else
			echo "ERROR: impl_version should be changed in ${VERSIONS_FILE}"
		fi
	fi
fi

exit 1
