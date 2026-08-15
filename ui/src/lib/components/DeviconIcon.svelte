<script lang="ts">
	import { deviconUrls } from '$lib/stores/services';
	import { deviconUrl } from '$lib/stores/services';

	interface Props {
		name: string;
		size?: 'sm' | 'md';
	}

	let { name, size = 'sm' }: Props = $props();

	// Logos drawn in solid black — inverted in dark mode so they stay visible.
	const MONOCHROME = new Set(['expo', 'vercel', 'express']);

	const px = $derived(size === 'md' ? 19 : 13);
	const url = $derived(deviconUrl($deviconUrls, name));
	const invert = $derived(MONOCHROME.has(name.toLowerCase()));
</script>

{#if url}
	<img
		src={url}
		alt={name}
		width={px}
		height={px}
		class="inline-block shrink-0 {invert ? 'dark:invert' : ''}"
		style="width: {px}px; height: {px}px;"
	/>
{/if}
