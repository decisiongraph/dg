<script lang="ts">
	import type { ComponentProps } from "svelte";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import NavMain from "./nav-main.svelte";
	import NavDocs from "./nav-docs.svelte";
	import { navData, navLoading } from "$lib/stores/nav";
	import BookOpenIcon from "@lucide/svelte/icons/book-open";
	import HomeIcon from "@lucide/svelte/icons/home";
	import NetworkIcon from "@lucide/svelte/icons/network";
	import KanbanIcon from "@lucide/svelte/icons/kanban";
	import MapIcon from "@lucide/svelte/icons/map";
	import BoxesIcon from "@lucide/svelte/icons/boxes";
	import LightbulbIcon from "@lucide/svelte/icons/lightbulb";
	import ShieldIcon from "@lucide/svelte/icons/shield";
	import AlertTriangleIcon from "@lucide/svelte/icons/triangle-alert";
	import FileStackIcon from "@lucide/svelte/icons/file-stack";
	import LoaderCircleIcon from "@lucide/svelte/icons/loader-circle";
	import type { NavItem } from "$lib/types";

	let { ref = $bindable(null), ...restProps }: ComponentProps<typeof Sidebar.Root> = $props();

	function isGroup(item: NavItem) {
		return item.children && item.children.length > 0;
	}

	/** Split nav into: leading leaves (before first group), groups, trailing leaves (after last group) */
	const leadingItems = $derived.by(() => {
		const items: NavItem[] = [];
		for (const item of $navData) {
			if (isGroup(item)) break;
			items.push(item);
		}
		return items;
	});
	const docGroup = $derived($navData.find((item: NavItem) => item.label === "Documents"));
	const otherGroups = $derived($navData.filter((item: NavItem) => isGroup(item) && item.label !== "Documents"));
	const footerItems = $derived.by(() => {
		const lastGroupIdx = $navData.findLastIndex(isGroup);
		if (lastGroupIdx === -1) return [];
		return $navData.slice(lastGroupIdx + 1).filter((item: NavItem) => !isGroup(item));
	});

	const leafIconMap: Record<string, typeof NetworkIcon> = {
		Introduction: HomeIcon,
		"Getting Started": BookOpenIcon,
		Graph: NetworkIcon,
		Kanban: KanbanIcon,
		Roadmap: MapIcon,
	};
</script>

<Sidebar.Root collapsible="icon" class="top-(--header-height) h-[calc(100svh-var(--header-height))]!" {...restProps}>
	<Sidebar.Content>
		{#if $navLoading}
			<!-- Static nav skeleton: shows structure immediately while data loads -->
			<Sidebar.Group>
				<Sidebar.GroupContent>
					<Sidebar.Menu>
						<Sidebar.MenuItem>
							<Sidebar.MenuButton tooltipContent="Introduction">
								{#snippet child({ props })}
									<a href="/" {...props}>
										<HomeIcon class="size-4" />
										<span>Introduction</span>
									</a>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
						<Sidebar.MenuItem>
							<Sidebar.MenuButton tooltipContent="Getting Started">
								{#snippet child({ props })}
									<a href="/onboarding/" {...props}>
										<BookOpenIcon class="size-4" />
										<span>Getting Started</span>
									</a>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
					</Sidebar.Menu>
				</Sidebar.GroupContent>
			</Sidebar.Group>
			<Sidebar.Group>
				<Sidebar.GroupLabel>
					<span class="flex items-center gap-2">
						<FileStackIcon class="size-4" />
						Documents
					</span>
				</Sidebar.GroupLabel>
				<Sidebar.GroupContent>
					<Sidebar.Menu>
						{#each [
							{ label: 'Architecture', href: '/architecture/', Icon: BoxesIcon },
							{ label: 'Opportunities', href: '/opportunities/', Icon: LightbulbIcon },
							{ label: 'Policies', href: '/policies/', Icon: ShieldIcon },
							{ label: 'Incidents', href: '/incidents/', Icon: AlertTriangleIcon },
						] as item (item.label)}
							<Sidebar.MenuItem>
								<Sidebar.MenuButton tooltipContent={item.label}>
									{#snippet child({ props })}
										<a href={item.href} {...props}>
											<item.Icon class="size-4" />
											<span>{item.label}</span>
										</a>
									{/snippet}
								</Sidebar.MenuButton>
								<Sidebar.MenuBadge>
									<LoaderCircleIcon class="size-3 animate-spin text-muted-foreground/50" />
								</Sidebar.MenuBadge>
							</Sidebar.MenuItem>
						{/each}
					</Sidebar.Menu>
				</Sidebar.GroupContent>
			</Sidebar.Group>
			<Sidebar.Group>
				<Sidebar.GroupContent>
					<Sidebar.Menu>
						<Sidebar.MenuItem>
							<Sidebar.MenuSkeleton />
						</Sidebar.MenuItem>
						<Sidebar.MenuItem>
							<Sidebar.MenuSkeleton />
						</Sidebar.MenuItem>
					</Sidebar.Menu>
				</Sidebar.GroupContent>
			</Sidebar.Group>
		{:else}
			{#if leadingItems.length > 0}
				<Sidebar.Group>
					<Sidebar.GroupContent>
						<Sidebar.Menu>
							{#each leadingItems as item (item.label)}
								{#if item.href != null}
									<Sidebar.MenuItem>
										<Sidebar.MenuButton tooltipContent={item.label}>
											{#snippet child({ props })}
												{@const LeadIcon = leafIconMap[item.label]}
												<a href="/{item.href}" {...props}>
													{#if LeadIcon}
														<LeadIcon class="size-4" />
													{/if}
													<span>{item.label}</span>
												</a>
											{/snippet}
										</Sidebar.MenuButton>
									</Sidebar.MenuItem>
								{/if}
							{/each}
						</Sidebar.Menu>
					</Sidebar.GroupContent>
				</Sidebar.Group>
			{/if}
			{#if docGroup}
				<NavDocs items={docGroup.children ?? []} />
			{/if}
			{#if otherGroups.length > 0}
				<NavMain items={otherGroups} />
			{/if}
		{/if}
	</Sidebar.Content>
	{#if footerItems.length > 0}
		<Sidebar.Footer>
			<Sidebar.Menu>
				{#each footerItems as item (item.label)}
					{#if item.href != null}
						<Sidebar.MenuItem>
							<Sidebar.MenuButton size="sm" tooltipContent={item.label}>
								{#snippet child({ props })}
									{@const FooterIcon = leafIconMap[item.label]}
									<a href="/{item.href}" {...props}>
										{#if FooterIcon}
											<FooterIcon class="size-4" />
										{/if}
										<span>{item.label}</span>
									</a>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
					{/if}
				{/each}
			</Sidebar.Menu>
		</Sidebar.Footer>
	{/if}
	<Sidebar.Rail />
</Sidebar.Root>
