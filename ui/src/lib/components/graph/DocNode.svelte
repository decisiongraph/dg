<script lang="ts">
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import { getContext } from 'svelte';
	import type { Writable } from 'svelte/store';
	import StatusBadge from '../StatusBadge.svelte';

	let { id, data }: NodeProps = $props();

	const highlightStore = getContext<Writable<Set<string>>>('graphHighlight');

	const typeColors: Record<string, string> = {
		adr: 'border-blue-400',
		opp: 'border-emerald-400',
		pol: 'border-purple-400',
		inc: 'border-red-400',
		spec: 'border-amber-400'
	};

	const typeBg: Record<string, string> = {
		adr: 'bg-blue-50 dark:bg-blue-950',
		opp: 'bg-emerald-50 dark:bg-emerald-950',
		pol: 'bg-purple-50 dark:bg-purple-950',
		inc: 'bg-red-50 dark:bg-red-950',
		spec: 'bg-amber-50 dark:bg-amber-950'
	};

	const borderClass = $derived(typeColors[data.docType as string] ?? 'border-border');
	const bgClass = $derived(typeBg[data.docType as string] ?? 'bg-muted');

	const inactiveStatuses = new Set(['declined', 'deprecated', 'superseded', 'retired', 'sunset', 'rejected']);
	const isInactive = $derived(inactiveStatuses.has((data.status as string)?.toLowerCase()));
	const dimmed = $derived(
		$highlightStore.size > 0 && !$highlightStore.has(id)
	);
</script>

<Handle type="target" position={Position.Top} class="!bg-muted-foreground !w-2 !h-2" />

<div
	class="rounded-lg border-2 bg-card shadow-sm px-3 py-1.5 w-[264px] transition-opacity duration-200 {borderClass}"
	style:opacity={dimmed ? 0.15 : isInactive ? 0.45 : 1}
	title={data.title as string}
>
	<div class="flex items-center gap-1.5 mb-0.5">
		<span class="font-mono text-[10px] leading-none text-muted-foreground rounded px-1 py-0.5 {bgClass}">{data.label as string}</span>
		{#if data.status}
			<StatusBadge status={data.status as string} docType={data.docType as string} class="!text-[10px] !leading-none !px-1.5 !py-0.5" />
		{/if}
	</div>
	<div class="text-xs text-foreground font-medium leading-tight truncate">
		{data.title}
	</div>
</div>

<Handle type="source" position={Position.Bottom} class="!bg-muted-foreground !w-2 !h-2" />
