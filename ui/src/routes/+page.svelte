<script lang="ts">
	import { allDocs, docsLoading } from '$lib/stores/docs';
	import { siteMeta, siteMetaLoading, readmeHtml, readmeSourcePath, readmeLoading } from '$lib/stores/site-meta';
	import { allServices, servicesLoading } from '$lib/stores/services';
	import DocCard from '$lib/components/DocCard.svelte';
	import ServiceCard from '$lib/components/ServiceCard.svelte';
	import HtmlContent from '$lib/components/HtmlContent.svelte';
	import SourceFileLink from '$lib/components/SourceFileLink.svelte';

	const ACTIVE_OPP_STATUSES = new Set(['pursuing', 'validating', 'proposed', 'planned', 'in-progress', 'active']);
	const activeOpps = $derived(
		$allDocs.filter((d) => d.type === 'opp' && ACTIVE_OPP_STATUSES.has(d.status?.toLowerCase()))
	);
</script>

<svelte:head>
	<title>{$siteMeta.title}</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	<!-- README section: loads separately from site-meta for faster initial render -->
	{#if $readmeLoading}
		<div class="mb-8 max-w-none rounded-xl border bg-card text-card-foreground p-6 shadow-sm space-y-4 animate-pulse">
			<div class="h-8 w-2/3 bg-muted rounded"></div>
			<div class="h-4 w-full bg-muted rounded"></div>
			<div class="h-4 w-5/6 bg-muted rounded"></div>
			<div class="h-4 w-4/6 bg-muted rounded"></div>
			<div class="h-6 w-1/3 bg-muted rounded mt-6"></div>
			<div class="h-4 w-full bg-muted rounded"></div>
			<div class="h-4 w-3/4 bg-muted rounded"></div>
		</div>
	{:else if $readmeHtml}
		<div class="relative prose prose-sm prose-gray dark:prose-invert mb-8 max-w-none rounded-xl border bg-card text-card-foreground p-6 shadow-sm">
			{#if $readmeSourcePath}
				<div class="float-right ml-4 not-prose">
					<SourceFileLink path={$readmeSourcePath} />
				</div>
			{/if}
			<HtmlContent html={$readmeHtml} />
		</div>
	{:else}
		<p class="text-muted-foreground mb-8">Browse decision documents, policies, and architecture records.</p>
	{/if}

	<!-- Services & Opportunities: load independently -->
	{#if $servicesLoading}
		<div class="grid gap-3 sm:grid-cols-2 mb-8">
			{#each [1, 2] as _}
				<div class="rounded-xl border bg-card p-4 animate-pulse">
					<div class="h-5 w-1/2 bg-muted rounded mb-2"></div>
					<div class="h-3 w-full bg-muted rounded mb-1"></div>
					<div class="h-3 w-3/4 bg-muted rounded"></div>
				</div>
			{/each}
		</div>
	{:else if $allServices.length > 0}
		{#each [['service', 'Services'], ['app', 'Apps'], ['infra', 'Infrastructure']] as [kind, label]}
			{@const items = $allServices.filter((s) => s.kind === kind)}
			{#if items.length > 0}
				<h2 class="text-lg font-semibold text-foreground mb-4">{label}</h2>
				<div class="grid gap-3 sm:grid-cols-2 mb-8">
					{#each items as service (service.slug)}
						<ServiceCard {service} />
					{/each}
				</div>
			{/if}
		{/each}
	{/if}

	{#if $docsLoading}
		<h2 class="text-lg font-semibold text-foreground mb-4">Active Opportunities</h2>
		<div class="grid gap-3 sm:grid-cols-2">
			{#each [1, 2, 3, 4] as _}
				<div class="rounded-xl border bg-card p-4 animate-pulse">
					<div class="h-4 w-1/3 bg-muted rounded mb-2"></div>
					<div class="h-5 w-3/4 bg-muted rounded mb-2"></div>
					<div class="h-3 w-1/2 bg-muted rounded"></div>
				</div>
			{/each}
		</div>
	{:else if activeOpps.length > 0}
		<h2 class="text-lg font-semibold text-foreground mb-4">Active Opportunities</h2>
		<div class="grid gap-3 sm:grid-cols-2">
			{#each activeOpps as doc}
				<DocCard {doc} />
			{/each}
		</div>
	{/if}
</div>
