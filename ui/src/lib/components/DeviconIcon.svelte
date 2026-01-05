<script lang="ts">
	import { deviconUrls } from '$lib/stores/services';
	import { deviconUrl } from '$lib/stores/services';

	interface Props {
		name: string;
		size?: 'sm' | 'md';
	}

	let { name, size = 'sm' }: Props = $props();

	const px = $derived(size === 'md' ? 24 : 16);
	const url = $derived(deviconUrl($deviconUrls, name));
</script>

{#if url}
	<img
		src={url}
		alt={name}
		width={px}
		height={px}
		class="inline-block shrink-0"
		style="width: {px}px; height: {px}px;"
	/>
{/if}
