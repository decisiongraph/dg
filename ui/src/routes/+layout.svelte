<script lang="ts">
	import '../app.css';
	import * as Sidebar from '$lib/components/ui/sidebar/index.js';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import SiteHeader from '$lib/components/site-header.svelte';
	import SearchCommand from '$lib/components/SearchCommand.svelte';
	import { onMount } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
	import { siteMeta } from '$lib/stores/site-meta';
	import { base } from '$app/paths';
	import { hideCard } from '$lib/actions/user-mentions';

	let { children } = $props();

	onMount(() => {
		// Remove pre-mount loading indicator now that SvelteKit has rendered
		document.getElementById('dg-loading')?.remove();
	});

	// Dismiss any open hover cards on navigation
	beforeNavigate(() => {
		hideCard(true);
	});

	$effect(() => {
		const logoUrl = $siteMeta.logo_url;
		if (logoUrl) {
			const link = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
			if (link) {
				link.href = `${base}/${logoUrl}`;
			}
		}
	});
</script>

<div class="[--header-height:calc(--spacing(14))]">
	<Sidebar.Provider class="flex flex-col">
		<SiteHeader />
		<div class="flex flex-1">
			<AppSidebar />
			<Sidebar.Inset>
				<main class="flex-1 p-6">
					{@render children()}
				</main>
			</Sidebar.Inset>
		</div>
	</Sidebar.Provider>
</div>

<SearchCommand />
