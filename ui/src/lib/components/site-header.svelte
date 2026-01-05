<script lang="ts">
	import SidebarIcon from "@lucide/svelte/icons/sidebar";
	import SearchIcon from "@lucide/svelte/icons/search";
	import SunIcon from "@lucide/svelte/icons/sun";
	import MoonIcon from "@lucide/svelte/icons/moon";
	import BookOpenIcon from "@lucide/svelte/icons/book-open";
	import { base } from "$app/paths";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Separator } from "$lib/components/ui/separator/index.js";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import { searchOpen } from "$lib/stores/search";
	import { isDark } from "$lib/stores/theme";
	import { siteMeta, siteMetaLoading } from "$lib/stores/site-meta";

	const sidebar = Sidebar.useSidebar();

	let dark = $state(false);

	function initTheme() {
		const stored = localStorage.getItem("theme");
		if (stored === "dark" || (!stored && window.matchMedia("(prefers-color-scheme: dark)").matches)) {
			dark = true;
		}
		applyTheme();
	}

	function applyTheme() {
		document.documentElement.classList.toggle("dark", dark);
		isDark.set(dark);
	}

	function toggleTheme() {
		dark = !dark;
		localStorage.setItem("theme", dark ? "dark" : "light");
		applyTheme();
	}

	$effect(() => {
		initTheme();
	});
</script>

<header class="bg-background sticky top-0 z-50 flex w-full items-center border-b" style="height: var(--header-height); padding-top: 0; padding-bottom: 0;">
	<div class="flex h-(--header-height) w-full items-center gap-2 px-4">
		<Button class="size-8" variant="ghost" size="icon" onclick={sidebar.toggle}>
			<SidebarIcon />
		</Button>
		<Separator orientation="vertical" class="mx-2 h-4" />
		<a href="/" class="flex items-center gap-2 text-sm font-medium text-foreground hover:text-foreground/80 shrink-0">
			{#if $siteMetaLoading}
				<div class="bg-muted flex size-6 items-center justify-center rounded-md animate-pulse"></div>
				<span class="hidden sm:inline h-4 w-24 bg-muted rounded animate-pulse"></span>
			{:else if $siteMeta.logo_url}
				<img src="{base}/{$siteMeta.logo_url}" alt="" class="size-6 rounded-md object-contain" />
				<span class="hidden sm:inline">{$siteMeta.title}</span>
			{:else}
				<div class="bg-primary text-primary-foreground flex size-6 items-center justify-center rounded-md">
					<BookOpenIcon class="size-3.5" />
				</div>
				<span class="hidden sm:inline">{$siteMeta.title}</span>
			{/if}
		</a>
		<button
			class="flex flex-1 items-center gap-2 rounded-lg border border-input bg-muted/50 px-3 py-1.5 text-sm text-muted-foreground hover:bg-accent ms-auto max-w-64"
			onclick={() => searchOpen.set(true)}
		>
			<SearchIcon class="size-4" />
			<span>Search...</span>
			<kbd class="ml-auto rounded bg-muted px-1.5 py-0.5 text-xs">&#8984;K</kbd>
		</button>
		<Button class="size-8" variant="ghost" size="icon" onclick={toggleTheme} aria-label="Toggle theme">
			{#if dark}
				<SunIcon class="size-4" />
			{:else}
				<MoonIcon class="size-4" />
			{/if}
		</Button>
	</div>
</header>
