<script lang="ts">
	import CopyIcon from '@lucide/svelte/icons/copy';
	import CheckIcon from '@lucide/svelte/icons/check';

	let { code = '' }: { code: string } = $props();
	let copied = $state(false);

	function copy() {
		navigator.clipboard.writeText(code).then(() => {
			copied = true;
			setTimeout(() => { copied = false; }, 2000);
		});
	}
</script>

<button
	onclick={copy}
	aria-label="Copy code"
	class="btn"
	class:copied
>
	{#if copied}
		<CheckIcon size={14} />
	{:else}
		<CopyIcon size={14} />
	{/if}
</button>

<style>
	.btn {
		position: absolute;
		top: 0.5rem;
		right: 0.5rem;
		z-index: 1;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.375rem;
		border-radius: 0.375rem;
		border: 1px solid var(--border);
		background: var(--muted);
		color: var(--muted-foreground);
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.15s, color 0.15s, background 0.15s;
	}
	:global(pre:hover) > :global([data-copy-btn]) > .btn {
		opacity: 1;
	}
	.btn:hover {
		background: var(--accent);
		color: var(--foreground);
	}
	.btn.copied {
		color: #16a34a;
		opacity: 1;
	}
</style>
