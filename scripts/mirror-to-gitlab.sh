#!/usr/bin/env bash
# Push the current branch to a GitLab mirror remote for team MR review.
# Usage: CANONIC_GITLAB_REMOTE=git@gitlab.example:group/canonic.git scripts/mirror-to-gitlab.sh [branch]
set -euo pipefail

remote_url="${CANONIC_GITLAB_REMOTE:?set CANONIC_GITLAB_REMOTE to the GitLab remote URL}"
branch="${1:-$(git rev-parse --abbrev-ref HEAD)}"
remote_name="gitlab-mirror"

if git remote get-url "$remote_name" >/dev/null 2>&1; then
    git remote set-url "$remote_name" "$remote_url"
else
    git remote add "$remote_name" "$remote_url"
fi

git push "$remote_name" "HEAD:refs/heads/$branch"
echo "pushed $branch to $remote_name ($remote_url); open the merge request on GitLab"
