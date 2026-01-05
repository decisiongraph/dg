<script lang="ts">
	import type { AssignmentsData, Assignment, DocsData, DocEntry } from '$lib/types';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import { enrichContentRefs } from '$lib/actions/content-refs';
	import { Button } from '$lib/components/ui/button/index.js';
	import { SvelteSet } from 'svelte/reactivity';

	let { data } = $props();

	const assignments = $derived(data.assignments as AssignmentsData | null);
	const docsData = $derived(data.docs as DocsData | null);

	const prefixToFolder: Record<string, string> = {
		adr: 'architecture',
		opp: 'opportunities',
		pol: 'policies',
		inc: 'incidents',
		spec: 'specifications'
	};

	function folderFor(docType: string): string {
		return prefixToFolder[docType] ?? docsData?.types[docType]?.folder ?? docType;
	}

	/** Priority rank for OPP status: pursuing > validating > identified > others */
	const oppStatusRank: Record<string, number> = {
		pursuing: 0,
		validating: 1,
		identified: 2
	};

	/** Priority rank for priority field */
	const priorityRank: Record<string, number> = {
		critical: 0,
		high: 1,
		medium: 2,
		low: 3
	};

	const priorityLabel: Record<number, string> = {
		0: 'Critical',
		1: 'High',
		2: 'Medium',
		3: 'Low'
	};

	const priorityColor: Record<number, string> = {
		0: 'bg-red-500',
		1: 'bg-orange-500',
		2: 'bg-blue-500',
		3: 'bg-slate-400'
	};

	interface KanbanTask {
		description: string;
		status: string;
		due_date?: string;
		doc_id: string;
		doc_type: string;
		doc_title: string;
		owner: string;
		section: string;
		parent_opp_id?: string;
		parent_opp_title?: string;
		opp_status_rank: number;
		priority_rank: number;
		source_order: number;
	}

	/** Build a lookup from doc_id to DocEntry */
	const docLookup = $derived.by(() => {
		const map: Record<string, DocEntry> = {};
		if (!docsData) return map;
		for (const doc of docsData.docs) {
			map[doc.id] = doc;
		}
		return map;
	});

	/** Pre-computed map from doc_id to its parent OPP */
	const parentOppMap = $derived.by(() => {
		const map = new Map<string, DocEntry>();
		if (!docsData) return map;
		for (const doc of docsData.docs) {
			if (doc.type === 'opp') {
				map.set(doc.id, doc);
				continue;
			}
			for (const ids of Object.values(doc.links)) {
				if (!ids) continue;
				for (const id of ids) {
					const linked = docLookup[id];
					if (linked?.type === 'opp') {
						map.set(doc.id, linked);
						break;
					}
				}
				if (map.has(doc.id)) break;
			}
			if (!map.has(doc.id)) {
				for (const bl of doc.backlinks) {
					const linked = docLookup[bl.id];
					if (linked?.type === 'opp') {
						map.set(doc.id, linked);
						break;
					}
				}
			}
		}
		return map;
	});

	/** All tasks extracted from assignments, deduplicated */
	const allTasks = $derived.by(() => {
		if (!assignments) return [];
		const tasks: KanbanTask[] = [];
		const seen = new Set<string>();
		let order = 0;

		for (const [handle, userAssignments] of Object.entries(assignments.users)) {
			for (const a of userAssignments) {
				if (a.role !== 'table_action_items' && a.role !== 'table_requirements') continue;
				const s = a.status?.toLowerCase();
				if (!s || s === 'completed') continue;

				const key = `${a.doc_id}:${a.section}:${a.description}`;
				if (seen.has(key)) continue;
				seen.add(key);

				const parentOpp = parentOppMap.get(a.doc_id);
				const showParentOpp = parentOpp && parentOpp.id !== a.doc_id;

				tasks.push({
					description: a.description ?? '',
					status: a.status ?? '',
					due_date: a.due_date,
					doc_id: a.doc_id,
					doc_type: a.doc_type,
					doc_title: a.doc_title,
					owner: handle,
					section: a.section ?? '',
					parent_opp_id: showParentOpp ? parentOpp.id : undefined,
					parent_opp_title: showParentOpp ? parentOpp.title : undefined,
					opp_status_rank: parentOpp
						? (oppStatusRank[parentOpp.status?.toLowerCase()] ?? 99)
						: 99,
					priority_rank: parentOpp?.priority
						? (priorityRank[parentOpp.priority.toLowerCase()] ?? 99)
						: 99,
					source_order: order++
				});
			}
		}

		tasks.sort((a, b) => {
			if (a.opp_status_rank !== b.opp_status_rank) return a.opp_status_rank - b.opp_status_rank;
			if (a.priority_rank !== b.priority_rank) return a.priority_rank - b.priority_rank;
			return a.source_order - b.source_order;
		});

		return tasks;
	});

	/** Unique owners sorted alphabetically */
	const allOwners = $derived(
		[...new Set(allTasks.map((t) => t.owner))].sort()
	);

	/** Selected users — empty means show all */
	let selectedUsers = new SvelteSet<string>();

	function toggleUser(user: string) {
		if (selectedUsers.has(user)) {
			selectedUsers.delete(user);
		} else {
			selectedUsers.add(user);
		}
	}

	function clearFilter() {
		selectedUsers.clear();
	}

	/** Filtered tasks based on selected users */
	const filteredTasks = $derived(
		selectedUsers.size === 0
			? allTasks
			: allTasks.filter((t) => selectedUsers.has(t.owner))
	);

	/** Kanban columns */
	const columns = [
		{ key: 'pending', label: 'Pending' },
		{ key: 'in-progress', label: 'In Progress' }
	] as const;

	function tasksForColumn(status: string): KanbanTask[] {
		return filteredTasks.filter((t) => t.status.toLowerCase() === status);
	}

	/** Group tasks that don't match any column */
	const otherTasks = $derived(
		filteredTasks.filter(
			(t) => !columns.some((c) => c.key === t.status.toLowerCase())
		)
	);
</script>

{#snippet taskCard(task: KanbanTask)}
	{@const folder = folderFor(task.doc_type)}
	<div class="rounded-lg border bg-card shadow-sm relative overflow-hidden">
		{#if task.priority_rank <= 3}
			<div class="absolute left-0 top-0 bottom-0 w-1 {priorityColor[task.priority_rank] ?? ''}"
				title="{priorityLabel[task.priority_rank] ?? ''} priority"></div>
		{/if}
		<div class="p-3 pl-4">
			<p class="text-sm font-medium text-foreground mb-2 leading-snug line-clamp-3" use:enrichContentRefs>{task.description}</p>
			{#if task.parent_opp_id}
				<div class="flex items-center gap-1.5 mb-1.5 text-xs text-muted-foreground">
					<a href="/opportunities/{task.parent_opp_id.toLowerCase()}"
						class="text-primary hover:underline font-mono">{task.parent_opp_id}</a>
					<span>{task.parent_opp_title}</span>
				</div>
			{/if}
			<div class="flex items-center gap-2 flex-wrap">
				<a
					href="/{folder}/{task.doc_id.toLowerCase()}"
					class="text-primary hover:underline font-mono text-xs"
				>{task.doc_id}</a>
				<span class="text-xs text-muted-foreground">{task.doc_title}</span>
			</div>
			<div class="flex items-center justify-between mt-2">
				<a href="/org/users/{task.owner}" class="text-primary hover:underline text-xs">@{task.owner}</a>
				{#if task.due_date}
					<span class="text-xs text-muted-foreground">{task.due_date}</span>
				{/if}
			</div>
		</div>
	</div>
{/snippet}

<svelte:head>
	<title>Kanban</title>
</svelte:head>

<div class="mx-auto max-w-7xl">
	<h1 class="text-2xl font-bold text-foreground mb-1">Kanban</h1>
	<p class="text-sm text-muted-foreground mb-4">
		Open tasks from action items and requirements across all documents, ordered by opportunity priority.
	</p>

	{#if allTasks.length === 0}
		<div class="text-muted-foreground">No open tasks found.</div>
	{:else}
		<!-- User filter bar -->
		{#if allOwners.length > 1}
			<div class="flex items-center gap-2 mb-4 flex-wrap">
				<span class="text-xs text-muted-foreground">Filter by person:</span>
				{#each allOwners as user (user)}
					{@const isSelected = selectedUsers.has(user)}
					<Button
						variant={isSelected ? 'default' : 'outline'}
						size="sm"
						onclick={() => toggleUser(user)}
						class="h-7 px-2.5 text-xs rounded-full"
					>
						@{user}
					</Button>
				{/each}
				{#if selectedUsers.size > 0}
					<Button
						variant="ghost"
						size="sm"
						onclick={clearFilter}
						class="h-7 px-2 text-xs text-muted-foreground"
					>
						Clear
					</Button>
				{/if}
			</div>
		{/if}

		<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
			{#each columns as col}
				{@const colTasks = tasksForColumn(col.key)}
				<div class="flex flex-col">
					<div class="flex items-center gap-2 mb-3 px-1">
						<StatusBadge status={col.key} />
						<span class="text-xs text-muted-foreground">{colTasks.length}</span>
					</div>
					<div class="flex flex-col gap-2 min-h-[200px] rounded-lg bg-muted/30 p-2">
						{#each colTasks as task (task.doc_id + ':' + task.section + ':' + task.description)}
							{@render taskCard(task)}
						{/each}
						{#if colTasks.length === 0}
							<div class="flex items-center justify-center h-full text-xs text-muted-foreground/50 py-8">
								No tasks
							</div>
						{/if}
					</div>
				</div>
			{/each}
		</div>

		{#if otherTasks.length > 0}
			<div class="mt-6">
				<h2 class="text-sm font-semibold text-foreground mb-3">Other statuses</h2>
				<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
					{#each [...new Set(otherTasks.map((t) => t.status.toLowerCase()))] as status}
						{@const colTasks = otherTasks.filter((t) => t.status.toLowerCase() === status)}
						<div class="flex flex-col">
							<div class="flex items-center gap-2 mb-3 px-1">
								<StatusBadge {status} />
								<span class="text-xs text-muted-foreground">{colTasks.length}</span>
							</div>
							<div class="flex flex-col gap-2 min-h-[100px] rounded-lg bg-muted/30 p-2">
								{#each colTasks as task (task.doc_id + ':' + task.section + ':' + task.description)}
									{@render taskCard(task)}
								{/each}
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	{/if}
</div>
