<script lang="ts">
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { schemaData } from '$lib/stores/schema';
	import LifecycleFlow from '$lib/components/LifecycleFlow.svelte';

	import type { SchemaEnumValue } from '$lib/types';

	interface Props {
		status: string;
		docType?: string;
		/** Override statuses for non-schema types (e.g. services) */
		overrideStatuses?: SchemaEnumValue[];
		class?: string;
	}

	let { status, docType, overrideStatuses, class: className = '' }: Props = $props();

	const colorMap: Record<string, string> = {
		active: 'bg-emerald-100 text-emerald-800 border-emerald-200',
		live: 'bg-emerald-100 text-emerald-800 border-emerald-200',
		pursuing: 'bg-amber-100 text-amber-800 border-amber-200',
		'in-progress': 'bg-amber-100 text-amber-800 border-amber-200',
		proposed: 'bg-amber-100 text-amber-800 border-amber-200',
		draft: 'bg-amber-100 text-amber-800 border-amber-200',
		beta: 'bg-amber-100 text-amber-800 border-amber-200',
		validating: 'bg-amber-100 text-amber-800 border-amber-200',
		investigating: 'bg-amber-100 text-amber-800 border-amber-200',
		mitigated: 'bg-amber-100 text-amber-800 border-amber-200',
		identified: 'bg-blue-100 text-blue-800 border-blue-200',
		planned: 'bg-blue-100 text-blue-800 border-blue-200',
		completed: 'bg-slate-100 text-slate-700 border-slate-200',
		accepted: 'bg-emerald-100 text-emerald-800 border-emerald-200',
		resolved: 'bg-emerald-100 text-emerald-800 border-emerald-200',
		declined: 'bg-red-100 text-red-800 border-red-200',
		rejected: 'bg-red-100 text-red-800 border-red-200',
		deprecated: 'bg-red-100 text-red-800 border-red-200',
		sunset: 'bg-red-100 text-red-800 border-red-200',
		superseded: 'bg-slate-100 text-slate-700 border-slate-200',
		parked: 'bg-blue-100 text-blue-800 border-blue-200',
		departed: 'bg-red-100 text-red-800 border-red-200'
	};

	/** Per-type overrides where the same status name needs a different color */
	const typeColorOverrides: Record<string, Record<string, string>> = {
		inc: { active: 'bg-red-100 text-red-800 border-red-200', open: 'bg-red-100 text-red-800 border-red-200' },
		opp: { completed: 'bg-emerald-100 text-emerald-800 border-emerald-200' }
	};

	const color = $derived(
		typeColorOverrides[docType ?? '']?.[status?.toLowerCase()] ??
		colorMap[status?.toLowerCase()] ??
		'bg-gray-100 text-gray-700 border-gray-200'
	);
	const label = $derived(status ? status.charAt(0).toUpperCase() + status.slice(1) : '');

	const typeInfo = $derived(docType ? $schemaData?.types[docType] : undefined);
	const statuses = $derived(overrideStatuses ?? typeInfo?.statuses ?? []);
	const currentDesc = $derived(statuses.find((s) => s.name === status?.toLowerCase())?.description);
	const hasTransitions = $derived(statuses.some((s) => s.transitions?.length));
	const hasTooltip = $derived(statuses.length > 0);
</script>

{#if hasTooltip}
	<Tooltip.Root delayDuration={300}>
		<Tooltip.Trigger class="cursor-default">
			<Badge variant="outline" class="{color} {className}">
				{label}
			</Badge>
		</Tooltip.Trigger>
		<Tooltip.Content side="bottom" sideOffset={4} class="{hasTransitions ? 'max-w-[420px]' : 'max-w-64'} pointer-events-none">
			{#if currentDesc}
				<p class="font-medium mb-1">{currentDesc}</p>
			{/if}
			{#if hasTransitions}
				<LifecycleFlow nodes={statuses} currentStatus={status} {docType} />
			{:else}
				<div class="flex flex-col gap-0.5">
					{#each statuses as s (s.name)}
						<span class="text-[11px] leading-snug {s.name === status?.toLowerCase() ? 'bg-accent text-accent-foreground rounded px-1 py-0.5 font-medium' : 'opacity-60 px-1 py-0.5'}">
							{s.name}{#if s.description && s.name !== status?.toLowerCase()} &mdash; {s.description}{/if}
						</span>
					{/each}
				</div>
			{/if}
		</Tooltip.Content>
	</Tooltip.Root>
{:else}
	<Badge variant="outline" class="{color} {className}">
		{label}
	</Badge>
{/if}
