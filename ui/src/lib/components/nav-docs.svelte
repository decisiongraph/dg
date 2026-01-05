<script lang="ts">
	import BoxesIcon from "@lucide/svelte/icons/boxes";
	import LightbulbIcon from "@lucide/svelte/icons/lightbulb";
	import ShieldIcon from "@lucide/svelte/icons/shield";
	import AlertTriangleIcon from "@lucide/svelte/icons/triangle-alert";
	import FileTextIcon from "@lucide/svelte/icons/file-text";
	import FolderIcon from "@lucide/svelte/icons/folder";
	import FileStackIcon from "@lucide/svelte/icons/file-stack";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import { page } from "$app/state";
	import type { NavItem } from "$lib/types";
	import type { Component } from "svelte";

	let { items }: { items: NavItem[] } = $props();

	const iconMap: Record<string, Component> = {
		Architecture: BoxesIcon,
		Opportunities: LightbulbIcon,
		Policies: ShieldIcon,
		Incidents: AlertTriangleIcon,
		Specifications: FileTextIcon,
	};

	/** Extract display name and count from label like "Architecture (5)" */
	function parseLabel(label: string): { name: string; count: string | null } {
		const match = label.match(/^(.+?)\s*\((\d+)\)$/);
		if (match) return { name: match[1], count: match[2] };
		return { name: label, count: null };
	}

	/** Active if current path starts with item href (highlights when viewing child docs) */
	function isActive(href: string | undefined): boolean {
		if (!href) return false;
		const norm = "/" + href.replace(/^\//, "").replace(/\/$/, "");
		const current = page.url.pathname.replace(/\/$/, "") || "/";
		return current === norm || current.startsWith(norm + "/");
	}
</script>

<Sidebar.Group>
	<Sidebar.GroupLabel>
		<span class="flex items-center gap-2">
			<FileStackIcon class="size-4" />
			Documents
		</span>
	</Sidebar.GroupLabel>
	<Sidebar.GroupContent>
		<Sidebar.Menu>
			{#each items as item (item.href ?? item.label)}
				{@const parsed = parseLabel(item.label)}
				{@const Icon = iconMap[parsed.name] ?? FolderIcon}
				<Sidebar.MenuItem>
					<Sidebar.MenuButton tooltipContent={parsed.name} isActive={isActive(item.href)}>
						{#snippet child({ props })}
							<a href="/{item.href}" {...props}>
								<Icon class="size-4" />
								<span>{parsed.name}</span>
							</a>
						{/snippet}
					</Sidebar.MenuButton>
					{#if parsed.count}
						<Sidebar.MenuBadge>{parsed.count}</Sidebar.MenuBadge>
					{/if}
				</Sidebar.MenuItem>
			{/each}
		</Sidebar.Menu>
	</Sidebar.GroupContent>
</Sidebar.Group>
