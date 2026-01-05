<script lang="ts">
	import { orgData, orgLoading } from '$lib/stores/org';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import Building2Icon from '@lucide/svelte/icons/building-2';

	const entities = $derived($orgData ? Object.entries($orgData.orgs) : []);
</script>

<svelte:head>
	<title>Entities</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	<Breadcrumb crumbs={[{ label: 'Entities' }]} />

	<h1 class="text-2xl font-bold text-foreground mb-6">Entities</h1>

	{#if $orgLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if entities.length === 0}
		<div class="text-muted-foreground">No entities configured.</div>
	{:else}
		<div class="grid gap-4 sm:grid-cols-2">
			{#each entities as [id, org] (id)}
				<a href="/org/{id}" class="rounded-xl border bg-card p-5 shadow-sm transition-all hover:shadow-md">
					<div class="flex items-center gap-3">
						<Building2Icon class="size-5 text-muted-foreground shrink-0" />
						<div>
							<h2 class="text-lg font-semibold text-foreground">{org.name}</h2>
							{#if org.parent}
								<div class="mt-1 text-sm text-muted-foreground">
									Parent: {$orgData?.orgs[org.parent]?.name ?? org.parent}
								</div>
							{/if}
							{#if org.children.length > 0}
								<div class="mt-1 text-sm text-muted-foreground">
									{org.children.length} {org.children.length === 1 ? 'subsidiary' : 'subsidiaries'}
								</div>
							{/if}
						</div>
					</div>
				</a>
			{/each}
		</div>
	{/if}
</div>
