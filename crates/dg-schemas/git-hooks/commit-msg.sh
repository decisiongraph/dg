#!/usr/bin/env bash
# DecisionGraph commit message validator
# Validates conventional commits with document references (Refs: DOC-001)
exec dg hooks commit-msg "$@"
