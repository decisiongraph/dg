<script lang="ts">
	import '../app.css';
	import Logo from '$lib/components/Logo.svelte';

	let { children } = $props();

	const REPO = 'https://github.com/decisiongraph/dg';
</script>

<a
	href="#main"
	class="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:rounded focus:bg-surface focus:px-3 focus:py-2"
	>Skip to content</a
>

<header class="mx-auto flex max-w-6xl flex-col items-center gap-4 px-4 py-4 sm:flex-row sm:justify-between sm:px-8">
	<a href="/" class="flex items-center gap-3 text-lg font-semibold text-fg no-underline">
		<Logo />
		<span>Decision Graph</span>
	</a>
	<nav aria-label="Main" class="flex flex-wrap items-center justify-center gap-x-6 gap-y-2 text-[0.95rem]">
		<a class="text-fg-2 hover:text-fg" href="/#features">Features</a>
		<a class="text-fg-2 hover:text-fg" href="/#demos">Demos</a>
		<a class="text-fg-2 hover:text-fg" href="/#record-types">Types</a>
		<a class="text-fg-2 hover:text-fg" href="/#install">Install</a>
		<a href={REPO} class="btn btn-outline">GitHub</a>
	</nav>
</header>

<main id="main">
	{@render children()}
</main>

<footer class="border-t border-line bg-bg-2 px-4 py-8 sm:px-8">
	<div class="mx-auto flex max-w-6xl flex-col items-center justify-between gap-4 sm:flex-row">
		<div class="flex items-center gap-2 font-medium text-fg-2">
			<Logo size={24} />
			<span>Decision Graph</span>
		</div>
		<nav aria-label="Footer" class="flex gap-8 text-sm">
			<a class="text-fg-2 hover:text-fg" href={REPO}>GitHub</a>
			<a class="text-fg-2 hover:text-fg" href="{REPO}/issues">Issues</a>
			<a class="text-fg-2 hover:text-fg" href="{REPO}/releases">Releases</a>
			<a class="text-fg-2 hover:text-fg" href="{REPO}#license">AGPL-3.0</a>
		</nav>
	</div>
</footer>
