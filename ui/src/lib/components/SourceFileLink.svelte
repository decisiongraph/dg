<script lang="ts">
	import { siteMeta } from '$lib/stores/site-meta';
	import FileCode2Icon from '@lucide/svelte/icons/file-code-2';
	import PencilIcon from '@lucide/svelte/icons/pencil';

	interface Props {
		path: string;
	}

	let { path }: Props = $props();

	const isLocalDev = $derived($siteMeta.is_local_dev ?? false);
	const editUrl = $derived(
		$siteMeta.edit_url_prefix ? `${$siteMeta.edit_url_prefix}${path}` : undefined
	);

	async function openLocally(e: MouseEvent) {
		if (!isLocalDev) return;
		e.preventDefault();
		await fetch(`/__dg/open?path=${encodeURIComponent(path)}`, { method: 'POST' });
	}
</script>

<span class="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
	<FileCode2Icon class="size-3.5 shrink-0" />
	<span class="font-mono">{path}</span>
	{#if isLocalDev}
		<button
			type="button"
			onclick={openLocally}
			class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
			title="Edit in editor"
		>
			<PencilIcon class="size-3" />
			Edit
		</button>
	{:else if editUrl}
		<a
			href={editUrl}
			target="_blank"
			rel="noopener noreferrer"
			class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground hover:text-foreground hover:bg-muted transition-colors no-underline"
			title="Edit on GitHub"
		>
			<PencilIcon class="size-3" />
			Edit
		</a>
	{/if}
</span>
