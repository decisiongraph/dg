<script lang="ts">
	// Keep in sync with the Install section of the repo README.md
	const tabs = [
		{ id: 'brew', label: 'Homebrew' },
		{ id: 'nix', label: 'Nix' },
		{ id: 'devenv', label: 'devenv' },
		{ id: 'cargo', label: 'Cargo' }
	] as const;

	let active = $state<(typeof tabs)[number]['id']>('brew');

	function onKeydown(e: KeyboardEvent) {
		const i = tabs.findIndex((t) => t.id === active);
		const next =
			e.key === 'ArrowRight' ? (i + 1) % tabs.length : e.key === 'ArrowLeft' ? (i + tabs.length - 1) % tabs.length : -1;
		if (next < 0) return;
		e.preventDefault();
		active = tabs[next].id;
		document.getElementById(`tab-${active}`)?.focus();
	}
</script>

<div class="mx-auto mt-12 max-w-2xl">
	<div
		role="tablist"
		aria-label="Installation method"
		tabindex="-1"
		class="flex flex-wrap justify-center gap-1 border-b border-line"
		onkeydown={onKeydown}
	>
		{#each tabs as tab (tab.id)}
			<button
				id="tab-{tab.id}"
				role="tab"
				type="button"
				aria-selected={active === tab.id}
				aria-controls="panel-{tab.id}"
				tabindex={active === tab.id ? 0 : -1}
				class="-mb-px cursor-pointer border-b-2 px-4 py-3 text-[0.95rem] font-medium transition-colors {active ===
				tab.id
					? 'border-primary text-primary'
					: 'border-transparent text-fg-2 hover:text-fg'}"
				onclick={() => (active = tab.id)}>{tab.label}</button
			>
		{/each}
	</div>

	<div class="mt-6 text-[0.95rem] [&_p]:mb-3 [&_p]:text-fg-2 [&_pre]:mb-4">
		<div id="panel-brew" role="tabpanel" aria-labelledby="tab-brew" hidden={active !== 'brew'}>
			<p>macOS and Linux:</p>
			<pre><code><span class="prompt">$</span> brew install decisiongraph/tap/dg</code></pre>
			<p>
				Or grab a prebuilt binary from
				<a href="https://github.com/decisiongraph/dg/releases">GitHub Releases</a>.
			</p>
		</div>

		<div id="panel-nix" role="tabpanel" aria-labelledby="tab-nix" hidden={active !== 'nix'}>
			<p>The repo is a Nix flake exposing a <code>dg</code> package:</p>
			<pre><code><span class="comment"># run without installing</span>
<span class="prompt">$</span> nix run github:decisiongraph/dg -- --version

<span class="comment"># install to your profile</span>
<span class="prompt">$</span> nix profile install github:decisiongraph/dg</code></pre>
			<p>
				Prebuilt binaries for x86_64-linux, aarch64-linux and aarch64-darwin are in the
				<a href="https://decisiongraph.cachix.org">decisiongraph cachix</a>:
				<code>cachix use decisiongraph</code>.
			</p>
		</div>

		<div id="panel-devenv" role="tabpanel" aria-labelledby="tab-devenv" hidden={active !== 'devenv'}>
			<p>Use the prebuilt release binaries (no compiling). In <code>devenv.yaml</code>:</p>
			<pre><code>inputs:
  dg:
    url: github:decisiongraph/dg
    flake: false</code></pre>
			<p>and in <code>devenv.nix</code>:</p>
			<pre><code>{"{"} pkgs, inputs, ... }: {"{"}
  packages = [ (pkgs.callPackage "$&#123;inputs.dg&#125;/nix/dg-bin.nix" {"{"} }) ];
}</code></pre>
			<p><code>devenv update dg</code> moves to the latest release.</p>
		</div>

		<div id="panel-cargo" role="tabpanel" aria-labelledby="tab-cargo" hidden={active !== 'cargo'}>
			<p>
				Build from source (needs Rust and <a href="https://bun.sh">bun</a> for the embedded web
				UI):
			</p>
			<pre><code><span class="prompt">$</span> cargo install --locked --git https://github.com/decisiongraph/dg dg-cli</code></pre>
			<p>Or from a checkout:</p>
			<pre><code><span class="prompt">$</span> git clone https://github.com/decisiongraph/dg && cd dg
<span class="prompt">$</span> cargo install --path crates/dg-cli</code></pre>
		</div>
	</div>
</div>
