<script lang="ts">
	import { page } from '$app/state';
	import { allServices, servicesLoading, loadServices } from '$lib/stores/services';
	import ServiceCard from '$lib/components/ServiceCard.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import { onMount } from 'svelte';

	onMount(() => loadServices());

	const kindSlug = $derived(page.params.kind ?? '');

	const kindMap: Record<string, { display: string; kindValue: string }> = {
		services: { display: 'Services', kindValue: 'service' },
		apps: { display: 'Apps', kindValue: 'app' },
		infra: { display: 'Infra', kindValue: 'infra' },
	};

	const kindInfo = $derived(kindMap[kindSlug]);
	const filtered = $derived(
		kindInfo ? $allServices.filter((s) => s.kind === kindInfo.kindValue) : []
	);
</script>

<svelte:head>
	<title>{kindInfo?.display ?? 'Software'}</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	<Breadcrumb crumbs={[{ label: 'Software' }, { label: kindInfo?.display ?? kindSlug }]} />

	<h1 class="text-2xl font-bold text-foreground mb-6">{kindInfo?.display ?? kindSlug}</h1>

	{#if $servicesLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if !kindInfo}
		<div class="text-muted-foreground">Unknown software category: {kindSlug}</div>
	{:else if filtered.length === 0}
		<div class="text-muted-foreground">No {kindInfo.display.toLowerCase()} found.</div>
	{:else}
		<div class="grid gap-3 sm:grid-cols-2">
			{#each filtered as service (service.slug)}
				<ServiceCard {service} />
			{/each}
		</div>
	{/if}
</div>
