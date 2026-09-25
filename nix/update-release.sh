#!/usr/bin/env bash
# Write nix/release.json (read by nix/dg-bin.nix) from a release's SHA256SUMS.
# Linux prefers the static musl tarball, falls back to glibc (pre-0.1.18).
#
# Usage: nix/update-release.sh <version> <SHA256SUMS> [out]
set -euo pipefail

version="$1"
sums="$2"
out="${3:-$(dirname "$0")/release.json}"

# hex sha256 of a release asset -> SRI hash, empty if the asset is missing
sri() {
  local hex bytes='' i
  hex="$(awk -v f="$1" '$2 == f { print $1 }' "$sums")"
  [[ -n "$hex" ]] || return 0
  for ((i = 0; i < ${#hex}; i += 2)); do bytes+="\\x${hex:i:2}"; done
  printf 'sha256-%s' "$(printf '%b' "$bytes" | base64 | tr -d '\n')"
}

assets='{}'
for pair in \
  aarch64-darwin:aarch64-apple-darwin \
  x86_64-darwin:x86_64-apple-darwin \
  aarch64-linux:aarch64-unknown-linux-musl,aarch64-unknown-linux-gnu \
  x86_64-linux:x86_64-unknown-linux-musl,x86_64-unknown-linux-gnu; do
  system="${pair%%:*}"
  IFS=, read -ra targets <<<"${pair#*:}"
  for target in "${targets[@]}"; do
    hash="$(sri "dg-${target}.tar.gz")"
    if [[ -n "$hash" ]]; then
      assets="$(jq --arg s "$system" --arg t "$target" --arg h "$hash" \
        '.[$s] = { target: $t, hash: $h }' <<<"$assets")"
      break
    fi
  done
  jq -e --arg s "$system" 'has($s)' <<<"$assets" >/dev/null \
    || { echo "no release asset for ${system} in ${sums}" >&2; exit 1; }
done

jq -n --arg v "$version" --argjson a "$assets" '{ version: $v, assets: $a }' >"$out"
