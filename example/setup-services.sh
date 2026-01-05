#!/usr/bin/env bash
# Initialize git repos in each service directory for per-service onefetch analysis.
# Run from example/ directory: bash setup-services.sh

set -euo pipefail

cd "$(dirname "$0")"

for dir in services/*/; do
  if [ ! -d "$dir/.git" ]; then
    echo "Initializing git repo in $dir"
    (cd "$dir" && git init && git add -A && git commit --no-gpg-sign -m "Initial commit")
  else
    echo "Skipping $dir (already initialized)"
  fi
done

echo "Done. All service repos initialized."
