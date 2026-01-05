<script lang="ts">
	import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
	import BoxesIcon from "@lucide/svelte/icons/boxes";
	import LightbulbIcon from "@lucide/svelte/icons/lightbulb";
	import ShieldIcon from "@lucide/svelte/icons/shield";
	import AlertTriangleIcon from "@lucide/svelte/icons/triangle-alert";
	import FileTextIcon from "@lucide/svelte/icons/file-text";
	import UsersIcon from "@lucide/svelte/icons/users";
	import UserIcon from "@lucide/svelte/icons/user";
	import HomeIcon from "@lucide/svelte/icons/home";
	import BookOpenIcon from "@lucide/svelte/icons/book-open";
	import FolderIcon from "@lucide/svelte/icons/folder";
	import Building2Icon from "@lucide/svelte/icons/building-2";
	import CodeIcon from "@lucide/svelte/icons/code";
	import ServerIcon from "@lucide/svelte/icons/server";
	import AppWindowIcon from "@lucide/svelte/icons/app-window";
	import CloudIcon from "@lucide/svelte/icons/cloud";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import * as Collapsible from "$lib/components/ui/collapsible/index.js";
	import { page } from "$app/state";
	import type { NavItem } from "$lib/types";
	import type { Component } from "svelte";

	let { items }: { items: NavItem[] } = $props();

	/** Map group labels to icons */
	const iconMap: Record<string, Component> = {
		Architecture: BoxesIcon,
		Opportunities: LightbulbIcon,
		Policies: ShieldIcon,
		Incidents: AlertTriangleIcon,
		Specifications: FileTextIcon,
		Organization: UsersIcon,
		Entities: Building2Icon,
		Introduction: HomeIcon,
		"Getting Started": BookOpenIcon,
		Software: CodeIcon,
	};

	/** Map child-level labels to icons */
	const childIconMap: Record<string, Component> = {
		Teams: UsersIcon,
		People: UserIcon,
		Entities: Building2Icon,
		Services: ServerIcon,
		Apps: AppWindowIcon,
		Infra: CloudIcon,
	};

	/** Strip internal marker prefixes from labels */
	function cleanLabel(label: string): string {
		return label.replace(/^(drawer|proposed|deprecated):/, "");
	}

	/** Extract display name and count from label like "Services (3)" */
	function parseLabel(label: string): { name: string; count: string | null } {
		const cleaned = cleanLabel(label);
		const match = cleaned.match(/^(.+?)\s*\((\d+)\)$/);
		if (match) return { name: match[1], count: match[2] };
		return { name: cleaned, count: null };
	}

	function isActive(href: string | undefined): boolean {
		if (href == null) return false;
		const current = page.url.pathname;
		const norm = "/" + href.replace(/^\//, "").replace(/index\.html$/, "").replace(/\/$/, "");
		const normCurrent = current.replace(/\/$/, "") || "/";
		return normCurrent === norm || normCurrent === norm + "/";
	}

	function hasActiveChild(item: NavItem): boolean {
		if (isActive(item.href)) return true;
		return item.children?.some((c) => hasActiveChild(c)) ?? false;
	}
</script>

{#each items as group (group.label)}
	{@const Icon = iconMap[cleanLabel(group.label)] ?? FolderIcon}
	<Sidebar.Group>
		<Sidebar.GroupLabel>
			{#if group.href}
				<a href="/{group.href}" class="flex items-center gap-2">
					<Icon class="size-4" />
					{cleanLabel(group.label)}
				</a>
			{:else}
				<span class="flex items-center gap-2">
					<Icon class="size-4" />
					{cleanLabel(group.label)}
				</span>
			{/if}
		</Sidebar.GroupLabel>
		<Sidebar.GroupContent>
			<Sidebar.Menu>
				{#each group.children ?? [] as navItem (navItem.href ?? navItem.label)}
					{@const parsed = parseLabel(navItem.label)}
					{@const ChildIcon = childIconMap[parsed.name]}
					{#if navItem.children && navItem.children.length > 0}
						<Collapsible.Root open={hasActiveChild(navItem)}>
							{#snippet child({ props })}
								<Sidebar.MenuItem {...props}>
									<Sidebar.MenuButton tooltipContent={parsed.name} isActive={isActive(navItem.href)}>
										{#snippet child({ props: btnProps })}
											{#if navItem.href}
												<a href="/{navItem.href}" {...btnProps}>
													{#if ChildIcon}<ChildIcon class="size-4" />{/if}
													<span>{parsed.name}</span>
												</a>
											{:else}
												<span {...btnProps}>
													{#if ChildIcon}<ChildIcon class="size-4" />{/if}
													<span>{parsed.name}</span>
												</span>
											{/if}
										{/snippet}
									</Sidebar.MenuButton>
									{#if parsed.count}
										<Collapsible.Trigger>
											{#snippet child({ props: triggerProps })}
												<Sidebar.MenuAction {...triggerProps}>
													<Sidebar.MenuBadge>{parsed.count}</Sidebar.MenuBadge>
													<span class="sr-only">Toggle</span>
												</Sidebar.MenuAction>
											{/snippet}
										</Collapsible.Trigger>
									{:else}
										<Collapsible.Trigger>
											{#snippet child({ props: triggerProps })}
												<Sidebar.MenuAction
													{...triggerProps}
													class="data-[state=open]:rotate-90"
												>
													<ChevronRightIcon />
													<span class="sr-only">Toggle</span>
												</Sidebar.MenuAction>
											{/snippet}
										</Collapsible.Trigger>
									{/if}
									<Collapsible.Content>
										<Sidebar.MenuSub>
											{#each navItem.children as subItem (subItem.href ?? subItem.label)}
												{#if subItem.href != null}
													<Sidebar.MenuSubItem>
														<Sidebar.MenuSubButton isActive={isActive(subItem.href)}>
															{#snippet child({ props: subProps })}
																<a href="/{subItem.href}" {...subProps} class={subItem.label.startsWith('deprecated:') ? 'opacity-60' : ''}>
																	<span>{cleanLabel(subItem.label)}</span>
																</a>
															{/snippet}
														</Sidebar.MenuSubButton>
													</Sidebar.MenuSubItem>
												{/if}
											{/each}
										</Sidebar.MenuSub>
									</Collapsible.Content>
								</Sidebar.MenuItem>
							{/snippet}
						</Collapsible.Root>
					{:else if navItem.href != null}
						<Sidebar.MenuItem>
							<Sidebar.MenuButton tooltipContent={parsed.name} isActive={isActive(navItem.href)}>
								{#snippet child({ props: btnProps })}
									<a href="/{navItem.href}" {...btnProps}>
										{#if ChildIcon}<ChildIcon class="size-4" />{/if}
										<span>{parsed.name}</span>
									</a>
								{/snippet}
							</Sidebar.MenuButton>
							{#if parsed.count}
								<Sidebar.MenuBadge>{parsed.count}</Sidebar.MenuBadge>
							{/if}
						</Sidebar.MenuItem>
					{/if}
				{/each}
			</Sidebar.Menu>
		</Sidebar.GroupContent>
	</Sidebar.Group>
{/each}
