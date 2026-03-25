<script lang="ts">
	import { page } from '$app/state';
	import { allDocs, docTypes, docsLoading } from '$lib/stores/docs';
	import { schemaData } from '$lib/stores/schema';
	import DocCard from '$lib/components/DocCard.svelte';
	import LifecycleFlow from '$lib/components/LifecycleFlow.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import type { DocEntry } from '$lib/types';

	/** Build folder→type mapping dynamically from docs.json types */
	const folderToType = $derived(
		Object.fromEntries(
			Object.entries($docTypes).map(([key, info]) => [info.folder, key])
		) as Record<string, string>
	);

	/** Fallback descriptions if schema.json doesn't have them */
	const fallbackDescriptions: Record<string, string> = {
		adr: 'Architecture Decision Records capture significant technical choices, their context, and consequences.',
		opp: 'Opportunities track business or technical initiatives being evaluated or pursued.',
		pol: 'Policies define rules, guidelines, and standards that govern how the organization operates.',
		inc: 'Incident reports document production issues, outages, and their resolution.',
		spec: 'Specifications describe detailed technical designs and requirements for features or systems.'
	};

	const typeSlug = $derived(page.params.type ?? '');
	const typeKey = $derived(folderToType[typeSlug] ?? typeSlug);
	const typeInfo = $derived($docTypes[typeKey]);
	const title = $derived(typeInfo?.display ?? typeSlug);

	const schemaType = $derived($schemaData?.types[typeKey]);
	const description = $derived(schemaType?.description ?? fallbackDescriptions[typeKey] ?? '');
	const statuses = $derived(schemaType?.statuses ?? []);

	/** Sort newest first by date, then by ID descending as tiebreaker */
	const sortedDocs = $derived(
		$allDocs
			.filter((d) => d.type === typeKey)
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
	<title>{title}</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	{#if $docsLoading}
		<div class="h-5 w-24 bg-muted rounded animate-pulse mb-4"></div>
		<div class="h-8 w-48 bg-muted rounded animate-pulse mb-2"></div>
		<div class="h-4 w-full bg-muted rounded animate-pulse mb-6"></div>
		<div class="grid gap-3 sm:grid-cols-2">
			{#each [1, 2, 3, 4] as _}
				<div class="rounded-xl border bg-card p-4 animate-pulse">
					<div class="h-4 w-1/3 bg-muted rounded mb-2"></div>
					<div class="h-5 w-3/4 bg-muted rounded mb-2"></div>
					<div class="h-3 w-1/2 bg-muted rounded"></div>
				</div>
			{/each}
		</div>
	{:else}
		<Breadcrumb crumbs={[{ label: title }]} />
		<h1 class="text-2xl font-bold text-foreground mb-2">{title}</h1>
		<p class="text-sm text-muted-foreground mb-4 leading-relaxed">
			<span class="font-medium">{sortedDocs.length}</span> {sortedDocs.length === 1 ? 'document' : 'documents'}{#if description}
				— {description}
			{/if}
		</p>
		{#if statuses.length > 0 && statuses.some(s => s.transitions?.length)}
			<div class="mb-4">
				<LifecycleFlow nodes={statuses} docType={typeKey} />
			</div>
		{/if}

		{#if sortedDocs.length === 0}
			<div class="text-muted-foreground">No documents found.</div>
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
