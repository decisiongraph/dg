<script lang="ts">
	import { page } from '$app/state';
	import { orgData, orgLoading } from '$lib/stores/org';
	import { assignmentsData, assignmentsLoading, loadAssignments, assignmentsForHandle } from '$lib/stores/assignments';
	import { allServices, loadServices } from '$lib/stores/services';
	import { docTypes } from '$lib/stores/docs';
	import UserAvatar from '$lib/components/UserAvatar.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import ServiceCardMini from '$lib/components/ServiceCardMini.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import SourceFileLink from '$lib/components/SourceFileLink.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import CircleCheckBigIcon from '@lucide/svelte/icons/circle-check-big';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import { onMount } from 'svelte';

	onMount(() => { loadAssignments(); loadServices(); });

	const handle = $derived(page.params.id ?? '');
	const user = $derived(handle ? $orgData?.users[handle] : undefined);
	const assignments = $derived(assignmentsForHandle($assignmentsData, handle));

	/** Services owned by this user's teams */
	const userServices = $derived.by(() => {
		if (!user) return [];
		const teamIds = new Set(user.teams);
		return $allServices.filter((s) => s.owner_team && teamIds.has(s.owner_team));
	});

	const services = $derived(assignments.filter((a) => a.role === 'service_owner'));
	/** Group doc-level roles by doc_id, merging roles into a single row */
	const docRoles = $derived.by(() => {
		const byDoc = new Map<string, { doc_id: string; doc_type: string; doc_title: string; status?: string; roles: string[] }>();
		for (const a of assignments) {
			if (!['author', 'owner', 'commander', 'approver'].includes(a.role)) continue;
			const existing = byDoc.get(a.doc_id);
			if (existing) {
				if (!existing.roles.includes(a.role)) existing.roles.push(a.role);
			} else {
				byDoc.set(a.doc_id, { doc_id: a.doc_id, doc_type: a.doc_type, doc_title: a.doc_title, status: a.status, roles: [a.role] });
			}
		}
		return [...byDoc.values()];
	});
	const incidentRoles = $derived(assignments.filter((a) =>
		['responders', 'commander'].includes(a.role) && a.doc_type === 'inc'
	));
	const allTableItems = $derived(assignments.filter((a) => a.role.startsWith('table_') && a.role !== 'table_timeline' && a.doc_type !== 'pol'));

	/** Unique incidents the user participated in */
	const incidents = $derived.by(() => {
		const seen = new Map<string, { doc_id: string; doc_type: string; doc_title: string; status?: string }>();
		for (const a of assignments) {
			if (a.doc_type !== 'inc') continue;
			if (seen.has(a.doc_id)) continue;
			seen.set(a.doc_id, { doc_id: a.doc_id, doc_type: a.doc_type, doc_title: a.doc_title, status: a.status });
		}
		return [...seen.values()];
	});

	function sortActionItems(items: typeof allTableItems): typeof allTableItems {
		return [...items].sort((a, b) => {
			const ad = a.due_date;
			const bd = b.due_date;
			// Items with due dates first, sorted by closest due date
			if (ad && bd) return ad.localeCompare(bd);
			if (ad && !bd) return -1;
			if (!ad && bd) return 1;
			// No due date: oldest doc_id first (lexicographic ≈ chronological)
			return a.doc_id.localeCompare(b.doc_id);
		});
	}

	const openItems = $derived(sortActionItems(allTableItems.filter((a) => a.status?.toLowerCase() !== 'completed')));
	const completedItems = $derived(sortActionItems(allTableItems.filter((a) => a.status?.toLowerCase() === 'completed')));
	let showCompleted = $state(false);

	function docHref(a: { doc_id: string; doc_type: string }): string {
		if (a.doc_type === 'service' || a.doc_type === 'app') return '#';
		const folder = $docTypes[a.doc_type]?.folder ?? a.doc_type;
		return `/${folder}/${a.doc_id.toLowerCase()}`;
	}

	function roleLabel(role: string): string {
		if (role.startsWith('table_')) {
			return role.replace('table_', '').replace(/_/g, ' ');
		}
		return role.replace(/_/g, ' ');
	}

	/** Action-item-specific status colors */
	const actionStatusColor: Record<string, string> = {
		completed: 'bg-emerald-100 text-emerald-800 border-emerald-200',
		pending: 'bg-yellow-50 text-yellow-700 border-yellow-200',
		'in-progress': 'bg-amber-100 text-amber-800 border-amber-200',
	};

	function actionStatusClass(status: string): string {
		return actionStatusColor[status?.toLowerCase()] ?? 'bg-gray-100 text-gray-700 border-gray-200';
	}

	function isOverdue(dueDate: string | undefined, status: string | undefined): boolean {
		if (!dueDate || status?.toLowerCase() === 'completed') return false;
		const today = new Date();
		today.setHours(0, 0, 0, 0);
		const due = new Date(dueDate + 'T00:00:00');
		return due <= today;
	}
</script>

<svelte:head>
	<title>{user?.name ?? handle ?? 'User'}</title>
</svelte:head>

<div class="mx-auto max-w-5xl space-y-6">
	{#if $orgLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if !user}
		<div class="text-muted-foreground">User not found: @{handle}</div>
	{:else}
		<div class="flex items-center justify-between mb-4 [&_nav]:mb-0">
			<Breadcrumb crumbs={[
				{ label: 'People', href: '/org/users' },
				{ label: user.name }
			]} />
			<SourceFileLink path=".dg/org.kdl" />
		</div>

		<div class="space-y-1">
			<div class="flex items-center gap-3">
				<UserAvatar {handle} name={user.name} avatarUrl={user.avatar_url} size="md" />
				<div>
					<h1 class="text-2xl font-bold tracking-tight text-foreground">{user.name}</h1>
					<div class="flex items-center gap-2 text-sm text-muted-foreground">
						<span class="font-mono">@{handle}</span>
						{#if user.title}
							<span>·</span>
							<span>{user.title}</span>
						{/if}
					</div>
				</div>
			</div>
			<Separator />
		</div>

		<div class="grid gap-4 sm:grid-cols-2">
			<Card.Root>
				<Card.Header class="pb-2">
					<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Details</Card.Title>
				</Card.Header>
				<Card.Content>
					<dl class="space-y-1.5 text-sm">
						{#if user.title}
							<div class="flex gap-2">
								<dt class="text-muted-foreground">Title:</dt>
								<dd class="text-foreground">{user.title}</dd>
							</div>
						{/if}
						{#if user.email}
							<div class="flex gap-2">
								<dt class="text-muted-foreground">Email:</dt>
								<dd><a href="mailto:{user.email}" class="text-primary hover:underline">{user.email}</a></dd>
							</div>
						{/if}
						<div class="flex gap-2">
							<dt class="text-muted-foreground">Status:</dt>
							<dd><StatusBadge status={user.status} /></dd>
						</div>
						<div class="flex gap-2">
							<dt class="text-muted-foreground">Kind:</dt>
							<dd>
								<Badge variant="outline" class={user.kind === 'ai' ? 'bg-violet-100 text-violet-800 border-violet-200' : user.kind === 'external' ? 'bg-blue-100 text-blue-800 border-blue-200' : 'bg-gray-100 text-gray-700 border-gray-200'}>
									{user.kind ?? 'internal'}
								</Badge>
							</dd>
						</div>
					</dl>
				</Card.Content>
			</Card.Root>

			{#if user.teams.length > 0}
				<Card.Root>
					<Card.Header class="pb-2">
						<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Teams</Card.Title>
					</Card.Header>
					<Card.Content>
						<div class="flex flex-wrap gap-2">
							{#each user.teams as teamId}
								<Badge variant="secondary" href="/org/teams/{teamId}">
									{$orgData?.teams[teamId]?.name ?? teamId}
								</Badge>
							{/each}
						</div>
					</Card.Content>
				</Card.Root>
			{/if}
		</div>

		{#if userServices.length > 0}
			<div>
				<h2 class="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-3">Services</h2>
				<div class="grid gap-3 sm:grid-cols-2">
					{#each userServices as svc (svc.slug)}
						<ServiceCardMini service={svc} />
					{/each}
				</div>
			</div>
		{/if}

		{#if !$assignmentsLoading}
			{#if docRoles.length > 0}
				<Card.Root>
					<Card.Header>
						<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Document Responsibilities</Card.Title>
					</Card.Header>
					<Card.Content class="p-0">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head class="px-4">Document</Table.Head>
									<Table.Head class="px-4">Role</Table.Head>
									<Table.Head class="px-4">Status</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each docRoles as a}
									<Table.Row>
										<Table.Cell class="px-4">
											<a href={docHref(a)} class="text-primary hover:underline font-medium">
												<span class="font-mono text-xs text-muted-foreground mr-1">{a.doc_id}</span>
												{a.doc_title}
											</a>
										</Table.Cell>
										<Table.Cell class="px-4">
											<div class="flex flex-wrap gap-1">
												{#each a.roles as role}
													<Badge variant="secondary" class="capitalize text-xs">{roleLabel(role)}</Badge>
												{/each}
											</div>
										</Table.Cell>
										<Table.Cell class="px-4">
											{#if a.status}
												<StatusBadge status={a.status} docType={a.doc_type} />
											{/if}
										</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</Card.Content>
				</Card.Root>
			{/if}

			{#if allTableItems.length > 0}
				<Card.Root>
					<Card.Header>
						<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Action Items & Assignments</Card.Title>
					</Card.Header>
					<Card.Content class="p-0">
						{#if completedItems.length > 0}
							<div class="px-4 pt-4 pb-2">
								<button
									type="button"
									class="w-full text-left"
									onclick={() => showCompleted = !showCompleted}
								>
									<Alert.Root class="border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-800 dark:bg-emerald-950/30 dark:text-emerald-300 cursor-pointer hover:bg-emerald-100 dark:hover:bg-emerald-950/50 transition-colors">
										<CircleCheckBigIcon class="text-emerald-600 dark:text-emerald-400" />
										<Alert.Title class="font-medium">{completedItems.length} item{completedItems.length === 1 ? '' : 's'} completed</Alert.Title>
										<Alert.Description class="text-emerald-700/70 dark:text-emerald-400/70 flex items-center gap-1">
											{showCompleted ? 'Click to hide' : 'Click to show'} completed work
											<ChevronDownIcon class="size-3.5 transition-transform {showCompleted ? 'rotate-180' : ''}" />
										</Alert.Description>
									</Alert.Root>
								</button>
							</div>
						{/if}

						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head class="px-4">Document</Table.Head>
									<Table.Head class="px-4">Item</Table.Head>
									<Table.Head class="px-4">Status</Table.Head>
									<Table.Head class="px-4 whitespace-nowrap">Due Date</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#if showCompleted}
									{#each completedItems as a}
										<Table.Row class="opacity-60">
											<Table.Cell class="px-4">
												<a href={docHref(a)} class="text-primary hover:underline">
													<span class="font-mono text-xs">{a.doc_id}</span>
												</a>
											</Table.Cell>
											<Table.Cell class="px-4 text-foreground">{a.description ?? roleLabel(a.role)}</Table.Cell>
											<Table.Cell class="px-4">
												<Badge variant="outline" class={actionStatusClass('completed')}>completed</Badge>
											</Table.Cell>
											<Table.Cell class="px-4 text-muted-foreground whitespace-nowrap">{a.due_date ?? ''}</Table.Cell>
										</Table.Row>
									{/each}
								{/if}
								{#each openItems as a}
									<Table.Row>
										<Table.Cell class="px-4">
											<a href={docHref(a)} class="text-primary hover:underline">
												<span class="font-mono text-xs">{a.doc_id}</span>
											</a>
										</Table.Cell>
										<Table.Cell class="px-4 text-foreground">{a.description ?? roleLabel(a.role)}</Table.Cell>
										<Table.Cell class="px-4">
											{#if a.status}
												<Badge variant="outline" class={actionStatusClass(a.status)}>{a.status}</Badge>
											{/if}
										</Table.Cell>
										<Table.Cell class="px-4 whitespace-nowrap {isOverdue(a.due_date, a.status) ? 'text-red-600 font-medium' : 'text-muted-foreground'}">{a.due_date ?? ''}</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</Card.Content>
				</Card.Root>
			{/if}

			{#if incidents.length > 0}
				<Card.Root>
					<Card.Header>
						<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Incidents</Card.Title>
					</Card.Header>
					<Card.Content class="p-0">
						<Table.Root>
							<Table.Header>
								<Table.Row>
									<Table.Head class="px-4">Incident</Table.Head>
									<Table.Head class="px-4">Title</Table.Head>
									<Table.Head class="px-4">Status</Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each incidents as inc}
									<Table.Row>
										<Table.Cell class="px-4">
											<a href={docHref(inc)} class="text-primary hover:underline">
												<span class="font-mono text-xs">{inc.doc_id}</span>
											</a>
										</Table.Cell>
										<Table.Cell class="px-4">
											<a href={docHref(inc)} class="text-foreground hover:underline">{inc.doc_title}</a>
										</Table.Cell>
										<Table.Cell class="px-4">
											{#if inc.status}
												<StatusBadge status={inc.status} docType={inc.doc_type} />
											{/if}
										</Table.Cell>
									</Table.Row>
								{/each}
							</Table.Body>
						</Table.Root>
					</Card.Content>
				</Card.Root>
			{/if}

			{#if assignments.length === 0}
				<div class="text-muted-foreground text-sm">No assignments found for this user.</div>
			{/if}
		{/if}
	{/if}
</div>
