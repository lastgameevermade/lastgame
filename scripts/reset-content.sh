#!/bin/sh
# Content rollback, the alpha's way out (CONSTITUTION.md). Core team only.
#
#   scripts/reset-content.sh <tag>        for example: scripts/reset-content.sh alpha-start
#
# Puts what the swarm builds (modules/, tests/, bench/) back to how it was at
# <tag> and keeps every platform fix made since: desk, checker, site, rules.
# It opens a core pull request and changes nothing else. After that is merged,
# POST /admin/season resets the desk, and POST /admin/phase starts the beta.
set -eu
tag=${1:?which tag, for example alpha-start}
cd "$(dirname "$0")/.."
[ -z "$(git status --porcelain)" ] || { echo "commit or stash your changes first"; exit 1; }
git fetch -q origin main --tags
git rev-parse -q --verify "refs/tags/$tag" > /dev/null || { echo "no tag $tag"; exit 1; }

branch="reset-to-$tag-$(date +%Y%m%d-%H%M)"
git switch -q -c "$branch" origin/main
for dir in modules tests bench; do
  git rm -r -q --ignore-unmatch -- "$dir"
  if git cat-file -e "$tag:$dir" 2> /dev/null; then
    git checkout "$tag" -- "$dir"
  fi
done
if git diff --cached --quiet; then
  echo "modules/, tests/ and bench/ are already as they were at $tag"
  git switch -q - && git branch -q -D "$branch"
  exit 0
fi
git diff --cached --stat | tail -n 1
git commit -q -m "Content rollback to $tag" -m "modules/, tests/ and bench/ go back to $tag. Platform fixes since then stay."
git push -q -u origin "$branch"
gh pr create --base main --head "$branch" --title "Content rollback to $tag" \
  --body "The alpha's way out (CONSTITUTION.md): modules/, tests/ and bench/ go back to \`$tag\`. Every platform fix made since stays.

After merging: \`POST /admin/season\` resets the desk, \`POST /admin/phase\` starts the beta."
