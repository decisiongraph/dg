<script lang="ts">
	import StatusBadge from './StatusBadge.svelte';
	import UserAvatar from './UserAvatar.svelte';
	import type { DocEntry } from '$lib/types';
	import { allDocs, docTypes } from '$lib/stores/docs';
	import { orgData } from '$lib/stores/org';
	import { isDone, isInactive } from '$lib/utils';

	interface Props {
		doc: DocEntry;
	}

	let { doc }: Props = $props();

	const typeColors: Record<string, string> = {
		adr: 'border-l-blue-500',
		opp: 'border-l-emerald-500',
		pol: 'border-l-violet-500',
		inc: 'border-l-red-500',
		spec: 'border-l-amber-500'
	};

	const neonGlow: Record<string, string> = {
		adr: '0 0 8px rgba(59,130,246,0.4)',
		opp: '0 0 8px rgba(16,185,129,0.4)',
		pol: '0 0 8px rgba(139,92,246,0.4)',
		inc: '0 0 8px rgba(239,68,68,0.4)',
		spec: '0 0 8px rgba(245,158,11,0.4)'
	};

	const borderColor = $derived(typeColors[doc.type] ?? 'border-l-gray-400');
	const glow = $derived(isInactive(doc.status) ? '' : (neonGlow[doc.type] ?? '0 0 8px rgba(156,163,175,0.4)'));
	const folder = $derived($docTypes[doc.type]?.folder ?? doc.type);
	const href = $derived(`/${folder}/${doc.id.toLowerCase()}`);
	const user = $derived(doc.author ? $orgData?.users[doc.author] : undefined);

	// Rollup over docs that implement / depend on this one
	const rollup = $derived.by(() => {
		const children = doc.backlinks.filter(
			(bl) => bl.relation === 'implements' || bl.relation === 'depends_on'
		);
		if (children.length < 2) return undefined;
		const done = children.filter((bl) =>
			isDone($allDocs.find((d) => d.id.toLowerCase() === bl.id.toLowerCase())?.status)
		).length;
		return { done, total: children.length };
	});
</script>

<a
	{href}
	class="doc-card block rounded-xl border border-l-4 {isInactive(doc.status) ? 'border-l-gray-300 opacity-50' : borderColor} bg-card p-4 shadow-sm transition-all hover:shadow-md"
	style="--neon-glow: {glow}"
>
	<div class="flex items-start justify-between gap-2">
		<div class="min-w-0">
			<span class="font-mono text-xs text-muted-foreground">{doc.id}</span>
			<h3 class="text-sm font-semibold text-foreground">{doc.title}</h3>
		</div>
		{#if doc.status}
			<StatusBadge status={doc.status} docType={doc.type} />
		{/if}
	</div>
	{#if doc.author || doc.date || rollup}
		<div class="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
			{#if doc.author}
				<UserAvatar handle={doc.author} name={user?.name ?? doc.author} avatarUrl={user?.avatar_url} size="sm" />
			{/if}
			{#if doc.date}<span>{doc.date}</span>{/if}
			{#if rollup}
				<span
					class="ml-auto rounded-full border px-2 py-0.5 text-[10px] font-medium {rollup.done === rollup.total ? 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-400' : 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-400'}"
					title="{rollup.done} of {rollup.total} implementing/dependent docs done"
				>
					{rollup.done}/{rollup.total} done
				</span>
			{/if}
		</div>
	{/if}
</a>

<style>
	:global(.dark) .doc-card:hover {
		box-shadow: var(--neon-glow);
	}
</style>
