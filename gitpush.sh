#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

# Usage: ./gitpush2.sh [commit message]
commit_msg=${1:-"Update"}

# Branch courante
branch=$(git rev-parse --abbrev-ref HEAD)
echo "Working on branch: $branch"

# Ensure .gitignore exists and ignore target/
if [ ! -f .gitignore ]; then
  touch .gitignore
fi
if ! git check-ignore -q -- "target/"; then
  echo "Adding 'target/' to .gitignore"
  if ! grep -qxF 'target/' .gitignore 2>/dev/null; then
    printf '\n# Rust build output\ntarget/\n' >> .gitignore
    git add .gitignore
    git commit -m "chore: ignore target/" || true
  fi
fi

# If files under target/ are currently tracked, remove them from index (keep on disk)
if git ls-files -- "target" "target/**" | grep -q .; then
  echo "Removing tracked files under target/ from the index..."
  git rm -r --cached target || true
  git commit -m "chore: remove target/ from repo (now ignored)" || true
fi

# Stage changes while excluding target/
set +e
if git add . -- ':!target' ':!target/**' 2>/dev/null; then
  echo "Staged changes excluding 'target/' (negative pathspec)."
else
  echo "Negative pathspec not supported — using fallback staging and unstaging target if needed."
  git add -A
  git reset -- 'target' 'target/**' >/dev/null 2>&1 || true
fi
set -e

# If nothing staged, exit
if git diff --cached --quiet; then
  echo "No changes to commit. Pulling and pushing remote state..."
  git pull --rebase origin "$branch"
  git push origin "$branch"
  echo "Done."
  exit 0
fi

# Commit staged changes
echo "Committing staged changes: $commit_msg"
git commit -m "$commit_msg"

# Pull with rebase and push
echo "Updating from origin/$branch (git pull --rebase)..."
git pull --rebase origin "$branch"

echo "Pushing commits to origin/$branch..."
git push origin "$branch"

echo "Finished. 'target/' is ignored and won't be pushed."
