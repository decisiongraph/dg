<script lang="ts">
	import DeviconIcon from './DeviconIcon.svelte';
	import { techInfo } from '$lib/stores/services';

	interface Props {
		name: string;
		label?: string;
		version?: string;
		class?: string;
	}

	let { name, label, version, class: className }: Props = $props();

	// JS-controlled visibility with a close-delay grace period: hovering the
	// tooltip cancels the close, and brief diagonal excursions outside the
	// pill on the way to the tooltip don't dismiss it.
	let open = $state(false);
	let closeTimer: ReturnType<typeof setTimeout> | undefined;
	function show() {
		clearTimeout(closeTimer);
		open = true;
	}
	function hide() {
		clearTimeout(closeTimer);
		closeTimer = setTimeout(() => (open = false), 250);
	}

	const info = $derived($techInfo[name.toLowerCase()]);
	const host = $derived.by(() => {
		if (!info) return '';
		try {
			return new URL(info.url).hostname.replace(/^www\./, '');
		} catch {
			return info.url;
		}
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
	class="relative inline-flex items-center gap-1 rounded-full bg-muted text-muted-foreground {className ??
		'px-2 py-0.5 text-xs'}"
	onmouseenter={show}
	onmouseleave={hide}
>
	<DeviconIcon {name} size="sm" />
	{label ?? name}
	{#if version}
		<span class="text-muted-foreground">{version}</span>
	{/if}
	{#if info}
		<!-- pb-1.5 (not margin) bridges the gap to the pill so the tooltip
		     stays hovered while the cursor travels up to it -->
		<span
			class="absolute bottom-full left-1/2 z-30 w-60 -translate-x-1/2 pb-1.5 transition-opacity duration-150 {open
				? 'pointer-events-auto opacity-100'
				: 'pointer-events-none opacity-0'}"
			role="tooltip"
		>
			<span
				class="block rounded-lg border border-border bg-popover p-2.5 text-left text-xs font-normal text-popover-foreground shadow-lg"
			>
				<span class="block font-semibold">{label ?? name}</span>
				<span class="mt-0.5 block text-muted-foreground">{info.description}</span>
				<a
					href={info.url}
					target="_blank"
					rel="noopener noreferrer"
					class="mt-1 block truncate text-primary hover:underline"
					onclick={(e) => e.stopPropagation()}
				>
					{host} ↗
				</a>
			</span>
		</span>
	{/if}
</span>
