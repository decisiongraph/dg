#!/usr/bin/env bash
# DecisionGraph commit message helper
# Auto-suggests document IDs from staged files
exec dg hooks prepare-commit-msg "$@"
