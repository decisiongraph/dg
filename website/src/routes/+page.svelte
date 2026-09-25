<script lang="ts">
	import Seo from '$lib/components/Seo.svelte';
	import InstallTabs from '$lib/components/InstallTabs.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const REPO = 'https://github.com/decisiongraph/dg';
	const records = $derived(data.schema.records);
	const relationNames = $derived(data.schema.relations.map((r) => r.name));

	const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
	const listText = (items: string[]) =>
		items.length <= 1
			? items.join('')
			: `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;

	const demos = $derived([
		{
			slug: 'pied-piper',
			name: 'Pied Piper',
			badge: 'Silicon Valley',
			gradient: 'linear-gradient(135deg, #00a55a, #007c43)',
			blurb:
				"The compression company's strategic decisions, from the middle-out algorithm architecture to the PiperNet decentralized internet.",
			stats: data.demos['pied-piper'],
			extra: null as string | null
		},
		{
			slug: 'microsoft',
			name: 'Microsoft',
			badge: 'Historical',
			gradient: 'linear-gradient(135deg, #0078d4, #004578)',
			blurb:
				"Historical decisions from Microsoft's founding through the Satya Nadella era. Based on the excellent",
			stats: data.demos.microsoft,
			extra: '1975–2014'
		}
	]);
</script>

<Seo />

<!-- Hero -->
<section class="bg-linear-to-b from-bg to-bg-2 px-4 pt-12 pb-20 text-center sm:px-8 sm:pt-16">
	<div class="mx-auto max-w-4xl">
		<h1
			class="mb-6 bg-linear-to-br from-fg to-fg-2 bg-clip-text text-[clamp(2.5rem,6vw,4rem)] leading-tight font-bold text-transparent"
		>
			Decisions as Code
		</h1>
		<p class="mx-auto mb-8 max-w-2xl text-xl leading-relaxed text-fg-2">
			A text-based knowledge graph for architecture decisions, policies, opportunities, incidents,
			specs and processes. Plain Markdown in your repo, validated against a schema, reviewed in
			pull requests.
		</p>
		<div class="mb-12 flex flex-col justify-center gap-4 sm:flex-row">
			<a href="#install" class="btn btn-primary justify-center">Get Started</a>
			<a href="/demo/pied-piper/" class="btn btn-secondary justify-center" data-sveltekit-reload
				>View Live Demo</a
			>
		</div>
		<div class="mx-auto max-w-2xl text-left">
			<pre class="rounded-xl p-6"><code
					><span class="prompt">$</span> dg init
<span class="prompt">$</span> dg new opportunity "Sell llama milk online"
<span class="out">OPP-001 -> docs/opportunities/opp-001-sell-llama-milk-online.md</span>

<span class="prompt">$</span> dg new adr "Use PostgreSQL" enables OPP-001
<span class="out">ADR-001 -> docs/architecture/adr-001-use-postgresql.md</span>

<span class="prompt">$</span> dg serve --open</code
				></pre>
		</div>
		<figure class="mx-auto mt-12 max-w-4xl">
			<img
				src="/screenshot-graph.png"
				width="1200"
				height="800"
				alt="Decision Graph web UI showing an interactive graph of linked ADRs, opportunities, policies and incidents"
				class="w-full rounded-xl border border-line shadow-2xl shadow-black/50"
				loading="lazy"
				decoding="async"
			/>
			<figcaption class="mt-3 text-sm text-fg-3">
				Explore your decision graph with <code>dg serve</code>
			</figcaption>
		</figure>
	</div>
</section>

<!-- Features -->
<section id="features" class="mx-auto max-w-6xl px-4 py-20 sm:px-8">
	<h2>Why Decision Graph?</h2>
	<div class="mt-12 grid gap-8 sm:grid-cols-2 lg:grid-cols-3">
		<div class="card p-6">
			<div class="mb-4 flex size-12 items-center justify-center rounded-lg bg-bg-3 text-primary">
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"
					><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" /><polyline points="14,2 14,8 20,8" /><line x1="16" y1="13" x2="8" y2="13" /><line x1="16" y1="17" x2="8" y2="17" /></svg
				>
			</div>
			<h3 class="mb-2 text-lg font-semibold">Plain Text &amp; Git-Native</h3>
			<p class="text-[0.95rem] text-fg-2">
				Markdown files with YAML frontmatter. Review decisions in PRs, track history with git blame,
				merge conflicts are just text.
			</p>
		</div>

		<div class="card p-6">
			<div class="mb-4 flex size-12 items-center justify-center rounded-lg bg-bg-3 text-primary">
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"
					><path d="M9 11l3 3L22 4" /><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" /></svg
				>
			</div>
			<h3 class="mb-2 text-lg font-semibold">{records.length} Record Types, One Schema</h3>
			<p class="text-[0.95rem] text-fg-2">
				{listText(records.map((r) => r.description))}. A KDL schema defines required fields, sections
				and status transitions; <code>dg validate</code> enforces it. Eject and add your own types.
			</p>
		</div>

		<div class="card p-6">
			<div class="mb-4 flex size-12 items-center justify-center rounded-lg bg-bg-3 text-primary">
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"
					><circle cx="6" cy="6" r="3" /><circle cx="18" cy="18" r="3" /><path d="M6 21V9a9 9 0 0 0 9 9" /></svg
				>
			</div>
			<h3 class="mb-2 text-lg font-semibold">Linked Knowledge Graph</h3>
			<p class="text-[0.95rem] text-fg-2">
				Connect records with typed relations: {listText(relationNames)}. Backlinks come for free;
				<code>dg lint</code> catches dangling refs, cycles and orphans.
			</p>
		</div>

		<div class="card p-6">
			<div class="mb-4 flex size-12 items-center justify-center rounded-lg bg-bg-3 text-primary">
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"
					><rect x="3" y="3" width="18" height="18" rx="2" ry="2" /><line x1="3" y1="9" x2="21" y2="9" /><line x1="9" y1="21" x2="9" y2="9" /></svg
				>
			</div>
			<h3 class="mb-2 text-lg font-semibold">Built-in Web UI</h3>
			<p class="text-[0.95rem] text-fg-2">
				<code>dg serve</code> launches a local site with search, an interactive graph, kanban,
				roadmap and team pages. <code>dg export --site</code> builds the same site as static files.
			</p>
		</div>

		<div class="card p-6">
			<div class="mb-4 flex size-12 items-center justify-center rounded-lg bg-bg-3 text-primary">
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"
					><path d="M12 2L2 7l10 5 10-5-10-5z" /><path d="M2 17l10 5 10-5" /><path d="M2 12l10 5 10-5" /></svg
				>
			</div>
			<h3 class="mb-2 text-lg font-semibold">Built for AI Agents</h3>
			<p class="text-[0.95rem] text-fg-2">
				<code>dg init</code> sets up skills and instructions for Claude Code, Gemini CLI and OpenCode.
				Agents write docs with <code>dg new</code> / <code>dg set</code> and get instant schema
				feedback from <code>dg validate</code>.
			</p>
		</div>

		<div class="card p-6">
			<div class="mb-4 flex size-12 items-center justify-center rounded-lg bg-bg-3 text-primary">
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"
					><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" /><circle cx="9" cy="7" r="4" /><path d="M23 21v-2a4 4 0 0 0-3-3.87" /><path d="M16 3.13a4 4 0 0 1 0 7.75" /></svg
				>
			</div>
			<h3 class="mb-2 text-lg font-semibold">Users &amp; Teams</h3>
			<p class="text-[0.95rem] text-fg-2">
				Define people, teams and orgs in <code>.dg/org.kdl</code>. Authors, owners and action-item
				assignees are validated, and departed users are tracked.
			</p>
		</div>
	</div>
</section>

<!-- Demos -->
<section id="demos" class="mx-auto max-w-6xl px-4 sm:px-8">
	<div class="rounded-xl bg-bg-2 px-4 py-16 sm:px-8">
		<h2>Live Demos</h2>
		<p class="mb-12 text-center text-fg-2">
			Explore decision archives built with Decision Graph
		</p>
		<div class="mx-auto grid max-w-3xl gap-8 md:grid-cols-2">
			{#each demos as demo (demo.slug)}
				<article
					class="card group relative overflow-hidden text-fg transition-transform hover:-translate-y-1 hover:shadow-xl hover:shadow-black/30"
				>
					<div class="relative h-24" style:background={demo.gradient}>
						<span class="absolute top-4 right-4 rounded-full bg-black/30 px-3 py-1 text-xs text-white"
							>{demo.badge}</span
						>
					</div>
					<div class="p-6">
						<h3 class="mb-2 text-xl font-semibold">
							<a
								href="/demo/{demo.slug}/"
								class="text-fg no-underline after:absolute after:inset-0"
								data-sveltekit-reload>{demo.name}</a
							>
						</h3>
						<p class="mb-4 text-[0.95rem] text-fg-2">
							{demo.blurb}
							{#if demo.slug === 'microsoft'}
								<a
									href="https://www.acquired.fm/episodes/microsoft"
									class="relative z-10 underline hover:text-fg">Acquired podcast</a
								> episodes.
							{/if}
						</p>
						<ul class="flex flex-wrap gap-2">
							<li class="pill">{plural(demo.stats.docs, 'record')}</li>
							{#if demo.stats.users}<li class="pill">{plural(demo.stats.users, 'person', 'people')}</li>{/if}
							{#if demo.stats.teams}<li class="pill">{plural(demo.stats.teams, 'team')}</li>{/if}
							{#if demo.extra}<li class="pill">{demo.extra}</li>{/if}
						</ul>
					</div>
				</article>
			{/each}
		</div>
	</div>
</section>

<!-- Record types: generated from crates/dg-schemas/schema.kdl -->
<section id="record-types" class="mx-auto max-w-6xl px-4 py-20 sm:px-8">
	<h2>Record Types</h2>
	<p class="text-center text-fg-2">
		{records.length} built-in types, straight from the
		<a href="{REPO}/blob/main/crates/dg-schemas/schema.kdl">default schema</a>.
	</p>
	<div class="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
		{#each records as t (t.name)}
			<div class="card rounded-lg p-4">
				<div class="mb-2 flex items-center gap-2">
					<span class="rounded bg-primary-strong px-2 py-0.5 font-mono text-xs font-semibold text-white"
						>{t.prefix}-001</span
					>
					<span class="font-mono text-xs text-fg-3">{t.folder}/</span>
				</div>
				<h3 class="mb-1 text-[0.95rem] font-semibold">{t.description}</h3>
				{#if t.requiredSections.length}
					<p class="text-sm text-fg-2">Sections: {t.requiredSections.join(' · ')}</p>
				{/if}
				{#if t.statuses.length}
					<p class="text-sm text-fg-3">Status: {t.statuses.join(' · ')}</p>
				{/if}
			</div>
		{/each}
	</div>
	{#if data.schema.singletons.length}
		<p class="mx-auto mt-8 max-w-2xl text-center text-sm text-fg-2">
			Plus {data.schema.singletons.length} README types
			({#each data.schema.singletons as s, i (s.name)}{i ? ', ' : ''}<code>{s.name}</code>{/each}):
			dg checks that project, service and app READMEs document architecture, risks and local
			development.
		</p>
	{/if}
</section>

<!-- Install -->
<section id="install" class="mx-auto max-w-6xl px-4 sm:px-8">
	<div class="rounded-xl bg-bg-2 px-4 py-16 sm:px-8">
		<h2>Installation</h2>
		<InstallTabs />

		<div class="mx-auto mt-12 max-w-2xl">
			<h3 class="mb-4 text-center text-lg font-semibold">Quick Start</h3>
			<pre><code
					><span class="comment"># Scaffold .dg/, git hooks and AI agent config</span>
<span class="prompt">$</span> dg init

<span class="comment"># Create linked records</span>
<span class="prompt">$</span> dg new opportunity "Sell llama milk online"
<span class="prompt">$</span> dg new adr "Use Rails with PostgreSQL" enables OPP-001
<span class="prompt">$</span> dg new spec "Llama milk checkout flow" implements OPP-001

<span class="comment"># Validate against the schema + graph health</span>
<span class="prompt">$</span> dg lint

<span class="comment"># Find things</span>
<span class="prompt">$</span> dg list --type adr
<span class="prompt">$</span> dg search "postgres"
<span class="prompt">$</span> dg refs OPP-001 --backlinks

<span class="comment"># Browse locally, or export a static site</span>
<span class="prompt">$</span> dg serve --open
<span class="prompt">$</span> dg export --site -o ./site</code
				></pre>
		</div>
	</div>
</section>

<!-- CTA -->
<section class="px-4 py-24 text-center sm:px-8">
	<h2>Start capturing decisions today</h2>
	<p class="mx-auto mb-8 max-w-lg text-fg-2">
		Your future self will thank you when you can trace why that architecture decision was made.
	</p>
	<div class="flex flex-col items-center justify-center gap-4 sm:flex-row">
		<a href={REPO} class="btn btn-primary">
			<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"
				><path
					d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"
				/></svg
			>
			View on GitHub
		</a>
		<a href="#install" class="btn btn-secondary">Install Now</a>
	</div>
</section>
