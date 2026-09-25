<script lang="ts">
	let { size = 32 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 100 100" fill="none" aria-hidden="true">
	<circle cx="30" cy="30" r="12" fill="currentColor" />
	<circle cx="70" cy="30" r="12" fill="currentColor" />
	<circle cx="50" cy="70" r="12" fill="currentColor" />
	<line x1="30" y1="30" x2="70" y2="30" stroke="currentColor" stroke-width="3" />
	<line x1="30" y1="30" x2="50" y2="70" stroke="currentColor" stroke-width="3" />
	<line x1="70" y1="30" x2="50" y2="70" stroke="currentColor" stroke-width="3" />
</svg>
