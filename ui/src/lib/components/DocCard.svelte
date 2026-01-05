<script lang="ts">
	import StatusBadge from './StatusBadge.svelte';
	import UserAvatar from './UserAvatar.svelte';
	import type { DocEntry } from '$lib/types';
	import { docTypes } from '$lib/stores/docs';
	import { orgData } from '$lib/stores/org';
	import { isInactive } from '$lib/utils';

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
	{#if doc.author || doc.date}
		<div class="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
			{#if doc.author}
				<UserAvatar handle={doc.author} name={user?.name ?? doc.author} avatarUrl={user?.avatar_url} size="sm" />
			{/if}
			{#if doc.date}<span>{doc.date}</span>{/if}
		</div>
	{/if}
</a>

<style>
	:global(.dark) .doc-card:hover {
		box-shadow: var(--neon-glow);
	}
</style>
