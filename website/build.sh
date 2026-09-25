#!/usr/bin/env bash
# Build decisiongraph.dev: the SvelteKit landing page plus one static dg site
# per demo project under build/demo/<name>/.
#
# Cloudflare Pages runs this as the build command (root directory: website/).
#
# Env:
#   DG_BIN      path to a dg binary to use (local dev / CI). If unset, the
#               x86_64 Linux musl binary of the latest GitHub release is
#               downloaded and verified against the release's SHA256SUMS.
#   DG_VERSION  release tag to download instead of the latest (e.g. v0.1.18)
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
WEBSITE_DIR=$PWD
OUT_DIR="$WEBSITE_DIR/build"
REPO="decisiongraph/dg"
ASSET="dg-x86_64-unknown-linux-musl.tar.gz"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

die() {
	echo "error: $*" >&2
	exit 1
}

download_dg() {
	local base
	if [ -n "${DG_VERSION:-}" ]; then
		base="https://github.com/$REPO/releases/download/$DG_VERSION"
	else
		base="https://github.com/$REPO/releases/latest/download"
	fi

	[ "$(uname -s)-$(uname -m)" = "Linux-x86_64" ] ||
		die "prebuilt dg download only supports Linux x86_64; set DG_BIN=/path/to/dg"

	echo "Downloading $ASSET from $base" >&2
	# The v3 Cloudflare Pages build image is Ubuntu 22.04 (glibc 2.35): the
	# glibc release builds need a newer glibc, so only the static musl build
	# works there. It first ships in v0.1.18.
	if ! curl -fsSL --retry 3 -o "$TMP_DIR/$ASSET" "$base/$ASSET"; then
		die "release asset $ASSET not found at $base.
The demos need a static musl dg binary (first published in dg v0.1.18) that
also includes subpath support for 'dg export --site' (PR #38).
Refusing to deploy a site with broken demo links."
	fi
	curl -fsSL --retry 3 -o "$TMP_DIR/SHA256SUMS" "$base/SHA256SUMS" ||
		die "SHA256SUMS not found at $base"

	(
		cd "$TMP_DIR"
		grep -E "^[0-9a-f]{64}  \*?$ASSET\$" SHA256SUMS >asset.sha256 ||
			die "$ASSET is not listed in SHA256SUMS"
		sha256sum -c asset.sha256 >&2 || die "checksum mismatch for $ASSET"
		tar -xzf "$ASSET"
	)
	local bin="$TMP_DIR/x86_64-unknown-linux-musl/dg"
	[ -x "$bin" ] || die "tarball did not contain x86_64-unknown-linux-musl/dg"
	echo "$bin"
}

if [ -n "${DG_BIN:-}" ]; then
	DG=$DG_BIN
	[ -x "$DG" ] || die "DG_BIN=$DG is not executable"
else
	DG=$(download_dg)
fi
dg_version=$("$DG" --version) || die "$DG does not run on this machine"
echo "Using $dg_version"

# 1. Landing page
bun install --frozen-lockfile
bun run build

# 2. Demos: one static dg site per project in demos/
for demo_dir in "$WEBSITE_DIR"/demos/*/; do
	name=$(basename "$demo_dir")
	dest="$OUT_DIR/demo/$name"
	title=$(sed -n 's/^# //p' "$demo_dir/README.md" | head -1)
	echo "Exporting demo '$name' -> build/demo/$name/"

	# Export from a copy outside the git checkout: the demo is not a repo of
	# its own, so this keeps "edit on GitHub" links and git history out of
	# the site, and dg's cache file out of the source tree.
	work="$TMP_DIR/$name"
	cp -R "$demo_dir" "$work"
	(cd "$work" && "$DG" export --site --no-git --title "${title:-$name}" -o "$dest")

	[ -f "$dest/index.html" ] || die "dg export produced no index.html for $name"
	# dg's SPA must work below /demo/<name>/ (runtime base detection, PR #38)
	grep -q "__dg_base" "$dest/index.html" ||
		die "$dg_version can't serve a site under a subpath (needs PR #38); demo links would break"

	# Unknown paths inside a demo: Cloudflare Pages serves the nearest
	# 404.html. dg writes an index.html for every real route, so anything
	# else is a genuine 404. (A copy of the SPA shell can't be used: its
	# runtime base detection only knows dg's own route names.)
	cat >"$dest/404.html" <<EOF
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Page not found | ${title:-$name}</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0a0a;color:#e5e5e5;font-family:system-ui,sans-serif;text-align:center}a{color:#60a5fa}</style>
</head>
<body><main><h1>404</h1><p>This page doesn't exist in the ${title:-$name} demo.</p><p><a href="/demo/$name/">Back to the demo</a> · <a href="/">decisiongraph.dev</a></p></main></body>
</html>
EOF
done

echo "Built $(find "$OUT_DIR" -type f | wc -l | tr -d ' ') files into $OUT_DIR"
