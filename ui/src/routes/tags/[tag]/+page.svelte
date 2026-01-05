<script lang="ts">
	import { page } from '$app/state';
	import { allDocs, docsLoading } from '$lib/stores/docs';
	import DocCard from '$lib/components/DocCard.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import type { DocEntry } from '$lib/types';

	const tag = $derived(decodeURIComponent(page.params.tag ?? ''));

	/** Sort newest first by date, then by ID descending as tiebreaker */
	const sortedDocs = $derived(
		$allDocs
			.filter((d) => d.tags?.some((t) => t.toLowerCase() === tag.toLowerCase()))
			.sort((a, b) => {
				const da = a.date ?? '';
				const db = b.date ?? '';
				if (da !== db) return db.localeCompare(da);
				return b.id.localeCompare(a.id);
			})
	);

	/** Group sorted docs by year, preserving order */
	const docsByYear = $derived.by(() => {
		const groups: { year: string; docs: DocEntry[] }[] = [];
		let currentYear = '';
		for (const doc of sortedDocs) {
			const year = doc.date?.slice(0, 4) ?? 'Undated';
			if (year !== currentYear) {
				currentYear = year;
				groups.push({ year, docs: [] });
			}
			groups[groups.length - 1].docs.push(doc);
		}
		return groups;
	});
</script>

<svelte:head>
	<title>#{tag}</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	{#if $docsLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else}
		<Breadcrumb crumbs={[{ label: `#${tag}` }]} />
		<h1 class="text-2xl font-bold text-foreground mb-2">#{tag}</h1>
		<p class="text-xs text-muted-foreground mb-6">{sortedDocs.length} documents</p>

		{#if sortedDocs.length === 0}
			<div class="text-muted-foreground">No documents with this tag.</div>
		{:else}
			{#each docsByYear as group (group.year)}
				<div class="mb-6">
					<h2 class="text-sm font-semibold text-muted-foreground uppercase tracking-wide mb-3">{group.year}</h2>
					<div class="grid gap-3 sm:grid-cols-2">
						{#each group.docs as doc (doc.id)}
							<DocCard {doc} />
						{/each}
					</div>
				</div>
			{/each}
		{/if}
	{/if}
</div>
