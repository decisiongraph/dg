<script lang="ts">
	import * as BC from '$lib/components/ui/breadcrumb/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import { allDocs, docTypes } from '$lib/stores/docs';

	interface Crumb {
		label: string;
		href?: string;
	}

	interface Props {
		crumbs: Crumb[];
	}

	let { crumbs }: Props = $props();

	/** For a type crumb (href like /architecture), get recent docs to show in dropdown */
	function docsForCrumb(href: string): { id: string; title: string; href: string }[] {
		const folder = href.replace(/^\//, '').replace(/\/$/, '');
		const typeEntry = Object.entries($docTypes).find(([, t]) => t.folder === folder);
		if (!typeEntry) return [];
		const typeKey = typeEntry[0];
		return $allDocs
			.filter((d) => d.type === typeKey)
			.slice(0, 8)
			.map((d) => ({ id: d.id, title: d.title, href: `/${folder}/${d.id.toLowerCase()}` }));
	}
</script>

<BC.Root class="mb-4">
	<BC.List>
		<BC.Item>
			<BC.Link href="/">Home</BC.Link>
		</BC.Item>
		{#each crumbs as crumb, i (i)}
			<BC.Separator />
			<BC.Item>
				{#if crumb.href}
					{@const docs = docsForCrumb(crumb.href)}
					{#if docs.length > 0}
						<DropdownMenu.Root>
							<DropdownMenu.Trigger class="flex items-center gap-1 hover:text-foreground transition-colors">
								{crumb.label}
								<svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
								</svg>
							</DropdownMenu.Trigger>
							<DropdownMenu.Content align="start">
								<DropdownMenu.Label>
									<a href={crumb.href} class="hover:underline">{crumb.label}</a>
								</DropdownMenu.Label>
								<DropdownMenu.Separator />
								{#each docs as doc (doc.id)}
									<DropdownMenu.Item>
										<a href={doc.href} class="block w-full">
											<span class="font-mono text-xs text-muted-foreground">{doc.id}</span>
											<span class="ml-2 text-sm">{doc.title}</span>
										</a>
									</DropdownMenu.Item>
								{/each}
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{:else}
						<BC.Link href={crumb.href}>{crumb.label}</BC.Link>
					{/if}
				{:else}
					<BC.Page>{crumb.label}</BC.Page>
				{/if}
			</BC.Item>
		{/each}
	</BC.List>
</BC.Root>
