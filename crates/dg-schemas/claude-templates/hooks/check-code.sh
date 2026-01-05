#!/usr/bin/env bash
# Check if edited files match decision doc code_paths.
# Delegates to dg hooks check-code for the actual logic.
# Runs as PostToolUse hook after Edit/Write operations.
if [ -n "$CLAUDE_FILE_PATH" ]; then
  exec dg hooks check-code "$CLAUDE_FILE_PATH"
fi
