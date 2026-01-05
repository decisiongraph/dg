<script lang="ts">
	import { searchOpen, searchQuery, searchResults } from '$lib/stores/search';
	import { docTypes } from '$lib/stores/docs';
	import { goto } from '$app/navigation';
	import * as Command from '$lib/components/ui/command/index.js';

	let open = $derived($searchOpen);

	function setOpen(v: boolean) {
		searchOpen.set(v);
		if (!v) searchQuery.set('');
	}

	let query = $state('');

	$effect(() => {
		searchQuery.set(query);
	});

	// Keyboard shortcut
	$effect(() => {
		function onKeydown(e: KeyboardEvent) {
			if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
				e.preventDefault();
				searchOpen.update((v) => !v);
			}
		}
		window.addEventListener('keydown', onKeydown);
		return () => window.removeEventListener('keydown', onKeydown);
	});

	function navigate(result: { id: string; type: string; href?: string }) {
		setOpen(false);
		query = '';
		if (result.href) {
			goto(result.href);
		} else {
			const folder = $docTypes[result.type]?.folder ?? result.type;
			goto(`/${folder}/${result.id.toLowerCase()}`);
		}
	}

	// Group results by type
	const groupedResults = $derived.by(() => {
		const results = $searchResults;
		const groups: Record<string, typeof results> = {};
		for (const r of results) {
			const type = r.type ?? 'other';
			if (!groups[type]) groups[type] = [];
			groups[type].push(r);
		}
		return groups;
	});

	const typeLabels: Record<string, string> = {
		adr: 'Architecture',
		opp: 'Opportunities',
		pol: 'Policies',
		inc: 'Incidents',
		spec: 'Specifications',
		user: 'People',
		team: 'Teams',
		entity: 'Organizations',
		service: 'Services'
	};
</script>

<Command.Dialog {open} onOpenChange={setOpen} title="Search" description="Search documents, people, teams and services">
	<Command.Input placeholder="Search..." bind:value={query} />
	<Command.List>
		<Command.Empty>No results found.</Command.Empty>
		{#each Object.entries(groupedResults) as [type, items] (type)}
			<Command.Group heading={typeLabels[type] ?? type}>
				{#each items as result (result.id)}
					<Command.Item
						value="{result.id} {result.title} {result.subtitle ?? ''}"
						onSelect={() => navigate(result)}
					>
						<div class="flex items-center gap-2 w-full min-w-0">
							{#if type === 'user'}
								<span class="shrink-0 w-5 h-5 rounded-full bg-muted flex items-center justify-center text-[10px] font-medium text-muted-foreground">
									{result.title.charAt(0).toUpperCase()}
								</span>
							{:else if type === 'team'}
								<span class="shrink-0 w-5 h-5 rounded bg-muted flex items-center justify-center text-[10px] text-muted-foreground">
									T
								</span>
							{:else if type === 'service'}
								<span class="shrink-0 w-2 h-2 rounded-full bg-emerald-500"></span>
							{:else}
								<span class="shrink-0 font-mono text-[10px] text-muted-foreground">{result.id}</span>
							{/if}
							<div class="flex flex-col min-w-0">
								<span class="truncate text-sm">{result.title}</span>
								{#if result.subtitle}
									<span class="truncate text-xs text-muted-foreground">{result.subtitle}</span>
								{/if}
							</div>
						</div>
					</Command.Item>
				{/each}
			</Command.Group>
		{/each}
	</Command.List>
</Command.Dialog>
