<script lang="ts">
	import type { RoadmapData } from '$lib/types';
	import { goto } from '$app/navigation';
	import { docTypes } from '$lib/stores/docs';
	import HtmlContent from '$lib/components/HtmlContent.svelte';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';

	let { data } = $props();
	const roadmapData = $derived(data.roadmap as RoadmapData | null);
	const generatedAt = $derived(roadmapData?.generated_at);

	let container: HTMLElement | undefined = $state();

	/** Map doc type prefix to folder for SPA routes */
	const prefixToFolder: Record<string, string> = {
		adr: 'architecture',
		opp: 'opportunities',
		pol: 'policies',
		inc: 'incidents',
		spec: 'specifications'
	};

	function resolveFolder(prefix: string): string {
		return prefixToFolder[prefix] ?? $docTypes[prefix]?.folder ?? prefix;
	}

	/** Scroll to the "today" marker and add tooltip to it */
	$effect(() => {
		if (!roadmapData?.html) return;
		requestAnimationFrame(() => {
			const marker = document.querySelector('.today-marker');
			if (marker) {
				marker.scrollIntoView({ inline: 'center', behavior: 'instant' });
				// Add tooltip showing the actual date
				if (generatedAt) {
					const label = marker.querySelector('.today-label');
					if (label) {
						(label as HTMLElement).title = generatedAt;
					}
				}
			}
		});
	});

	/** Attach click delegation to the pre-rendered roadmap container (anchors are the real targets) */
	$effect(() => {
		const el = container;
		if (!el) return;
		el.addEventListener('click', handleClick);
		return () => el.removeEventListener('click', handleClick);
	});

	/** Intercept clicks on .html links from pre-rendered roadmap and navigate via SPA */
	function handleClick(event: MouseEvent) {
		const anchor = (event.target as HTMLElement).closest('a');
		if (!anchor) return;
		const href = anchor.getAttribute('href');
		if (!href || !href.endsWith('.html')) return;

		// Extract doc ID from href like "opp-001.html" or "index.html"
		const docId = href.replace(/\.html$/, '');
		if (docId === 'index') return; // let index.html navigate normally

		// Determine type prefix (e.g. "opp" from "opp-001")
		const prefix = docId.split('-')[0];
		const folder = resolveFolder(prefix);

		event.preventDefault();
		goto(`/${folder}/${docId}`);
	}
</script>

<svelte:head>
	<title>Roadmap</title>
</svelte:head>

<div class="mx-auto max-w-6xl">
	<div class="flex items-baseline gap-3 mb-4">
		{#if generatedAt}
			<Tooltip.Root>
				<Tooltip.Trigger class="cursor-default">
					<h1 class="text-2xl font-bold text-foreground">Roadmap</h1>
				</Tooltip.Trigger>
				<Tooltip.Content>
					<p>Generated {generatedAt}</p>
				</Tooltip.Content>
			</Tooltip.Root>
		{:else}
			<h1 class="text-2xl font-bold text-foreground">Roadmap</h1>
		{/if}
	</div>

	{#if roadmapData?.html}
		<div class="roadmap-container rounded-xl border bg-card p-6 shadow-sm" bind:this={container}>
			<HtmlContent html={roadmapData.html} />
		</div>
	{:else}
		<div class="text-muted-foreground">No roadmap data available.</div>
	{/if}
</div>

<style>
	/* Override backlog cards to match SPA DocCard design */
	.roadmap-container :global(.backlog-section) {
		margin-top: 1.5rem;
	}
	.roadmap-container :global(.backlog-title) {
		font-size: 1rem;
		font-weight: 600;
		color: hsl(var(--foreground));
	}
	.roadmap-container :global(.backlog-count) {
		font-size: 0.7rem;
	}
	.roadmap-container :global(.item-card) {
		background: hsl(var(--card));
		border: 1px solid hsl(var(--border));
		border-left: 4px solid #10b981;
		border-radius: 0.75rem;
		padding: 1rem;
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
		transition: box-shadow 0.15s, border-color 0.15s;
	}
	.roadmap-container :global(.item-card:hover) {
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
	}
	/* Split the title into mono ID + bold title like DocCard */
	.roadmap-container :global(.item-header) {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 0.5rem;
	}
	.roadmap-container :global(.item-title) {
		font-size: 0.875rem;
		font-weight: 600;
		color: hsl(var(--foreground));
		line-height: 1.4;
	}
	.roadmap-container :global(.item-title a) {
		color: inherit;
		text-decoration: none;
	}
	.roadmap-container :global(.item-title a:hover) {
		text-decoration: underline;
	}
	.roadmap-container :global(.status-badge) {
		font-size: 0.75rem;
		font-weight: 500;
		border-radius: 9999px;
		padding: 0.125rem 0.5rem;
		flex-shrink: 0;
	}
	.roadmap-container :global(.item-badges) {
		margin-top: 0.5rem;
	}
</style>
