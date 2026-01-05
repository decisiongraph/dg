<script lang="ts">
	import StatusBadge from './StatusBadge.svelte';
	import DeviconIcon from './DeviconIcon.svelte';
	import type { ServiceEntry } from '$lib/types';

	interface Props {
		service: ServiceEntry;
	}

	let { service }: Props = $props();

	const statusColors: Record<string, string> = {
		live: 'border-l-emerald-500',
		beta: 'border-l-amber-500',
		planned: 'border-l-blue-500',
		sunset: 'border-l-red-500',
		deprecated: 'border-l-red-500'
	};

	const borderColor = $derived(statusColors[service.status.toLowerCase()] ?? 'border-l-gray-400');
</script>

<a
	href="/services/{service.slug}"
	class="block rounded-lg border border-l-4 {borderColor} bg-card p-3 shadow-sm transition-all hover:shadow-md no-underline text-inherit"
>
	<div class="flex items-center justify-between gap-2">
		<div class="flex items-center gap-2 min-w-0">
			<DeviconIcon name={service.primary_language} size="md" />
			<span class="text-sm font-semibold text-foreground truncate">{service.name}</span>
		</div>
		<StatusBadge status={service.status} />
	</div>
	{#if service.description}
		<p class="mt-1 text-xs text-muted-foreground line-clamp-1">{service.description}</p>
	{/if}
	{#if service.frameworks?.length > 0}
		<div class="mt-1.5 flex flex-wrap gap-1">
			{#each service.frameworks as fw (fw)}
				<span class="inline-flex items-center gap-1 rounded-full bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
					<DeviconIcon name={fw} size="sm" />
					{fw}
				</span>
			{/each}
		</div>
	{/if}
</a>
