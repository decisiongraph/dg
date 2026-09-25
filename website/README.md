# decisiongraph.dev

Marketing site for Decision Graph: a prerendered SvelteKit app (Svelte 5, Tailwind v4, `adapter-static`) plus two live demos generated with `dg export --site`.

```
website/
├── src/                 # SvelteKit landing page
│   └── lib/schema.ts    # reads ../crates/dg-schemas/schema.kdl at build time
├── static/              # favicon, robots.txt, _headers, OG screenshot
├── demos/
│   ├── pied-piper/      # dg project (.dg/ + docs/) → /demo/pied-piper/
│   └── microsoft/       # dg project (.dg/ + docs/) → /demo/microsoft/
└── build.sh             # Cloudflare Pages build command
```

The record types section and its count come from the built-in schema (`crates/dg-schemas/schema.kdl`), parsed by a small KDL parser during prerendering, so the page can't drift from what `dg` ships. Demo stats (records, people, teams) are counted from `demos/*` the same way.

## Development

```bash
bun install
bun run dev          # landing page only, http://localhost:5173
bun run check        # svelte-check
bun run test         # KDL/schema parser tests
```

Full build including the demos, with a local dg:

```bash
cargo build -p dg-cli                       # from the repo root, inside `devenv shell`
DG_BIN=../target/debug/dg ./build.sh        # → build/
bunx serve build                            # or any static server
```

Without `DG_BIN`, `build.sh` downloads `dg-x86_64-unknown-linux-musl.tar.gz` from the latest [GitHub release](https://github.com/decisiongraph/dg/releases) (or `DG_VERSION`), verifies it against `SHA256SUMS` and uses that. This only works on Linux x86_64. The build fails if the asset is missing or if that dg can't generate a site that works below a subpath (`/demo/<name>/`).

### Demos

Each demo is a normal dg project. Edit docs with `dg` from inside the demo directory:

```bash
cd demos/pied-piper
dg lint && dg fmt --check
```

The demos add a project-local `odr` type (Organizational Decision Record) in `.dg/schema.kdl`: a copy of the built-in schema plus that type, which holds the business decisions from the original samples. CI lints both demos and builds the site with the dg from the same commit.

## Deployment (Cloudflare Pages)

The `decisiongraph-dev` Pages project is connected to this repo through the Git integration, so Cloudflare builds and deploys every push. There's no deploy workflow or API token in GitHub. Dashboard settings (Workers & Pages → decisiongraph-dev → Settings → Build):

| Setting | Value |
|---|---|
| Production branch | `main` |
| Framework preset | None |
| Build command | `./build.sh` |
| Build output directory | `build` |
| Root directory (advanced) | `website` |
| Build system version | v3 |
| Build watch paths: include | `website/*` |
| Build watch paths: exclude | *(empty)* |
| Environment variable | `BUN_VERSION` = `1.3.13` (Production and Preview) |

Optional: set `DG_VERSION` (e.g. `v0.1.18`) to pin the dg release used for the demos instead of the latest one.

Notes:

- There's no `wrangler.toml` on purpose: for Pages it would become the source of truth and make these dashboard settings read-only, and a static site doesn't need it.
- The v3 build image is Ubuntu 22.04 (glibc 2.35). dg's glibc release binaries need a newer glibc, so the build uses the static musl binary, first published in **v0.1.18**. Until that release exists the build fails with a clear error instead of deploying broken demos.
- Demo links under `/demo/<name>/` need the subpath support from [#38](https://github.com/decisiongraph/dg/pull/38) in the released dg. `build.sh` checks for it.
- The watch paths skip commits that don't touch `website/`. A new dg release doesn't trigger a rebuild on its own: use **Retry deployment** on the latest production deployment (or a deploy hook) to regenerate the demos with it.
- `404.html` (SvelteKit fallback) handles unknown paths on the landing page; each demo gets its own `demo/<name>/404.html`. `static/_headers` sets long-lived caching for content-hashed `_app/immutable/` assets.
