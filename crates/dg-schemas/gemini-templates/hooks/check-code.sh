#!/usr/bin/env bash
# Check if edited files match decision doc code_paths.
# Delegates to dg hooks check-code for the actual logic.
if [ -n "$1" ]; then
  exec dg hooks check-code "$1"
fi
