#!/usr/bin/env bash
# Minimal wrapper - delegates to dg hooks check-fixme
# This allows upgrading dg to get new functionality
exec dg hooks check-fixme "$@"
