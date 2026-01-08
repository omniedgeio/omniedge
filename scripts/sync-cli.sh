#!/bin/bash

# Configuration
REMOTE_NAME="cli-origin"
REMOTE_URL="https://github.com/omniedgeio/omniedge-cli.git"
PREFIX="omniedge-cli"
BRANCH="main"

# Ensure remote exists
if ! git remote | grep -q "^$REMOTE_NAME$"; then
    echo "Adding remote $REMOTE_NAME..."
    git remote add "$REMOTE_NAME" "$REMOTE_URL"
fi

# Fetch tags and data
echo "Fetching from $REMOTE_NAME..."
git fetch "$REMOTE_NAME" --tags

# If a version is provided as an argument, use it; otherwise use BRANCH
VERSION=${1:-$BRANCH}

echo "Syncing $PREFIX with $VERSION from $REMOTE_NAME..."

# Perform the subtree pull
git subtree pull --prefix="$PREFIX" "$REMOTE_NAME" "$VERSION" --squash

if [ $? -eq 0 ]; then
    echo "Successfully synced $PREFIX with $VERSION"
else
    echo "Error: Sync failed. Please check for conflicts."
    exit 1
fi
