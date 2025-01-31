#!/bin/bash
# This script will use the open dependabot PRs to perform an upgrade of all GH actions

# The target branch to merge all Dependabot PRs into
TARGET_BRANCH="upgrade-gh-actions"

# Ensure the target branch exists and is checked out
git checkout -b $TARGET_BRANCH 2>/dev/null || git checkout $TARGET_BRANCH

# Fetch all PRs from GitHub, filter for those opened by Dependabot, and extract their branch names
gh pr list --search "author:app/dependabot" --state open --json headRefName --jq '.[].headRefName' | while read branch; do
    # Merge each Dependabot branch into the target branch
    echo "Merging $branch into $TARGET_BRANCH..."
    git merge origin/$branch --no-edit
done

# After merging, you might want to push the changes
# git push origin $TARGET_BRANCH
