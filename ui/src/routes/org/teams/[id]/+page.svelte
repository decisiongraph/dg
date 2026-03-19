<script lang="ts">
	import { page } from '$app/state';
	import { orgData, orgLoading } from '$lib/stores/org';
	import { assignmentsData, assignmentsLoading, loadAssignments, assignmentsForHandles } from '$lib/stores/assignments';
	import { allServices, servicesLoading, loadServices } from '$lib/stores/services';
	import { docTypes } from '$lib/stores/docs';
	import UserAvatar from '$lib/components/UserAvatar.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import ServiceCardMini from '$lib/components/ServiceCardMini.svelte';
	import HtmlContent from '$lib/components/HtmlContent.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import SourceFileLink from '$lib/components/SourceFileLink.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import Building2Icon from '@lucide/svelte/icons/building-2';
	import UsersIcon from '@lucide/svelte/icons/users';
	import HashIcon from '@lucide/svelte/icons/hash';
	import LinkIcon from '@lucide/svelte/icons/link';
	import NetworkIcon from '@lucide/svelte/icons/network';
	import UserXIcon from '@lucide/svelte/icons/user-x';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import { onMount } from 'svelte';

	onMount(() => { loadAssignments(); loadServices(); });

	const teamId = $derived(page.params.id ?? '');
	const team = $derived(teamId ? $orgData?.teams[teamId] : undefined);
	const teamOrg = $derived(team?.org ? $orgData?.orgs[team.org] : undefined);
	const parentTeam = $derived(team?.parent ? $orgData?.teams[team.parent] : undefined);
	const childTeams = $derived(
		(team?.children ?? []).map((id) => ({ id, team: $orgData?.teams[id] })).filter((c) => c.team)
	);
	const extraEntries = $derived(Object.entries(team?.extra ?? {}));

	const KNOWN_EXTRA: Record<string, { label: string; url?: (v: string) => string }> = {
		slack: { label: 'Slack' },
		pagerduty: { label: 'PagerDuty', url: (v) => v },
		wiki: { label: 'Wiki', url: (v) => v },
		jira_board: { label: 'Jira', url: (v) => v },
	};

	const assignments = $derived(
		team ? assignmentsForHandles($assignmentsData, team.members) : []
	);

	/** Services owned by this team (matched via owner_team field) */
	const teamServices = $derived(
		$allServices.filter((s) => s.owner_team === teamId)
	);

	const activeMembers = $derived(
		(team?.members ?? []).filter((h) => $orgData?.users[h]?.status !== 'departed')
	);
	const departedMembers = $derived(
		(team?.members ?? []).filter((h) => $orgData?.users[h]?.status === 'departed')
	);
	let showDeparted = $state(false);

	const services = $derived(assignments.filter((a) => a.role === 'service_owner'));
	const openItems = $derived(assignments.filter((a) => {
		if (!a.role.startsWith('table_')) return false;
		if (a.doc_type === 'inc' || a.doc_type === 'pol') return false;
		const s = a.status?.toLowerCase() ?? '';
		return s !== 'done' && s !== 'completed' && s !== 'closed';
	}));

	/** Unique incidents the team participated in */
	const incidents = $derived.by(() => {
		const seen = new Map<string, { doc_id: string; doc_type: string; doc_title: string; status?: string }>();
		for (const a of assignments) {
			if (a.doc_type !== 'inc') continue;
			if (seen.has(a.doc_id)) continue;
			seen.set(a.doc_id, { doc_id: a.doc_id, doc_type: a.doc_type, doc_title: a.doc_title, status: a.status });
		}
		return [...seen.values()];
	});

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
</script>

<svelte:head>
	<title>{team?.name ?? teamId ?? 'Team'}</title>
</svelte:head>

<div class="mx-auto max-w-5xl space-y-6">
	{#if $orgLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if !team}
		<div class="text-muted-foreground">Team not found: {teamId}</div>
	{:else}
		<div class="flex items-center justify-between mb-4 [&_nav]:mb-0">
			<Breadcrumb crumbs={[
				{ label: 'Teams', href: '/org/teams' },
				{ label: team.name }
			]} />
			<SourceFileLink path={team.source_path ?? '.dg/org.kdl'} />
		</div>

		<div class="space-y-1">
			<div class="flex items-start justify-between">
				<div class="space-y-1">
					<h1 class="text-2xl font-bold tracking-tight text-foreground">
						{team.name}
						{#if team.status === 'deprecated'}
							<Badge variant="outline" class="ml-2 bg-muted text-muted-foreground align-middle text-xs">deprecated</Badge>
						{/if}
					</h1>
					{#if teamOrg || parentTeam}
						<div class="flex items-center gap-3 text-sm text-muted-foreground">
							{#if teamOrg}
								<span class="inline-flex items-center gap-1.5">
									<Building2Icon class="size-3.5 shrink-0" />
									<a href="/org/{team.org}" class="text-primary hover:underline">{teamOrg.name}</a>
								</span>
							{/if}
							{#if parentTeam}
								<span class="inline-flex items-center gap-1.5">
									<NetworkIcon class="size-3.5 shrink-0" />
									<a href="/org/teams/{team.parent}" class="text-primary hover:underline">{parentTeam.name}</a>
								</span>
							{/if}
						</div>
					{/if}
				</div>
				{#if team.lead}
					{@const leadUser = $orgData?.users[team.lead]}
					<div class="flex items-center gap-2 rounded-full border bg-muted/50 px-3 py-1.5 text-sm">
						<span class="text-muted-foreground">Lead:</span>
						<a href="/org/users/{team.lead}" class="inline-flex items-center gap-1.5 text-primary hover:underline no-underline">
							<UserAvatar handle={team.lead} name={leadUser?.name ?? team.lead} avatarUrl={leadUser?.avatar_url} size="sm" />
							<span>{leadUser?.name ?? `@${team.lead}`}</span>
						</a>
					</div>
				{/if}
			</div>
			<Separator />
		</div>

		{#if extraEntries.length > 0}
			<div class="flex flex-wrap gap-2">
				{#each extraEntries as [key, value]}
					{@const known = KNOWN_EXTRA[key]}
					{#if known?.url}
						<a href={known.url(value)} target="_blank" rel="noopener"
							class="inline-flex items-center gap-1.5 rounded-full border bg-muted/50 px-3 py-1 text-xs font-medium text-foreground hover:bg-muted transition-colors">
							<LinkIcon class="size-3 shrink-0 text-muted-foreground" />
							{known.label}
						</a>
					{:else if key === 'slack'}
						<span class="inline-flex items-center gap-1.5 rounded-full border bg-muted/50 px-3 py-1 text-xs font-medium text-foreground">
							<HashIcon class="size-3 shrink-0 text-muted-foreground" />
							{value}
						</span>
					{:else}
						<span class="inline-flex items-center gap-1.5 rounded-full border bg-muted/50 px-3 py-1 text-xs font-medium text-muted-foreground">
							{key}: {value}
						</span>
					{/if}
				{/each}
			</div>
		{/if}

		{#if team.body_html}
			<Card.Root>
				<Card.Header>
					<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Onboarding</Card.Title>
				</Card.Header>
				<Card.Content>
					<div class="prose prose-sm dark:prose-invert max-w-none">
						<HtmlContent html={team.body_html} />
					</div>
				</Card.Content>
			</Card.Root>
		{/if}

		{#if childTeams.length > 0}
			<div>
				<h2 class="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-3">Sub-teams</h2>
				<div class="grid gap-3 sm:grid-cols-2">
					{#each childTeams as { id, team: child } (id)}
						<a href="/org/teams/{id}" class="block rounded-lg border bg-card p-4 hover:bg-muted/50 transition-colors">
							<div class="flex items-center gap-2">
								<UsersIcon class="size-4 text-muted-foreground shrink-0" />
								<span class="font-medium text-foreground">{child?.name ?? id}</span>
							</div>
							{#if child?.description}
								<p class="mt-1 text-sm text-muted-foreground line-clamp-2">{child.description}</p>
							{/if}
							{#if child?.members?.length}
								<p class="mt-1 text-xs text-muted-foreground">{child.members.length} member{child.members.length === 1 ? '' : 's'}</p>
							{/if}
						</a>
					{/each}
				</div>
			</div>
		{/if}

		<Card.Root>
			<Card.Header>
				<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Members</Card.Title>
			</Card.Header>
			{#if activeMembers.length === 0 && departedMembers.length === 0}
				<Card.Content>
					<div class="text-muted-foreground">No members.</div>
				</Card.Content>
			{:else}
				<Card.Content class="p-0">
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head class="px-4">Handle</Table.Head>
								<Table.Head class="px-4">Name</Table.Head>
								<Table.Head class="px-4">Title</Table.Head>
								<Table.Head class="px-4">Status</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each activeMembers as handle}
								{@const user = $orgData?.users[handle]}
								<Table.Row>
									<Table.Cell class="px-4">
										<a href="/org/users/{handle}" class="inline-flex items-center gap-2 text-primary hover:underline font-medium">
											<UserAvatar {handle} name={user?.name ?? handle} avatarUrl={user?.avatar_url} size="sm" />
											@{handle}
										</a>
									</Table.Cell>
									<Table.Cell class="px-4 text-foreground">{user?.name ?? handle}</Table.Cell>
									<Table.Cell class="px-4 text-muted-foreground">{user?.title ?? ''}</Table.Cell>
									<Table.Cell class="px-4">
										<Badge variant="outline" class={(user?.status ?? '') === 'active' ? 'bg-emerald-100 text-emerald-800 border-emerald-200' : 'bg-muted text-muted-foreground'}>
											{user?.status ?? ''}
										</Badge>
									</Table.Cell>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</Card.Content>
			{/if}
		</Card.Root>

		{#if departedMembers.length > 0}
			<button
				type="button"
				class="w-full text-left"
				onclick={() => showDeparted = !showDeparted}
			>
				<Alert.Root class="border-gray-200 bg-gray-50 text-gray-600 dark:border-gray-700 dark:bg-gray-900/30 dark:text-gray-400 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/50 transition-colors">
					<UserXIcon class="text-gray-500 dark:text-gray-500" />
					<Alert.Title class="font-medium">{departedMembers.length} departed {departedMembers.length === 1 ? 'member' : 'members'}</Alert.Title>
					<Alert.Description class="text-gray-500/70 dark:text-gray-500/70 flex items-center gap-1">
						{showDeparted ? 'Click to hide' : 'Click to show'} departed members
						<ChevronDownIcon class="size-3.5 transition-transform {showDeparted ? 'rotate-180' : ''}" />
					</Alert.Description>
				</Alert.Root>
			</button>

			{#if showDeparted}
				<div class="rounded-lg border bg-card opacity-60">
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head class="px-4">Handle</Table.Head>
								<Table.Head class="px-4">Name</Table.Head>
								<Table.Head class="px-4">Title</Table.Head>
								<Table.Head class="px-4">Status</Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each departedMembers as handle}
								{@const user = $orgData?.users[handle]}
								<Table.Row>
									<Table.Cell class="px-4">
										<a href="/org/users/{handle}" class="inline-flex items-center gap-2 text-primary hover:underline font-medium">
											<UserAvatar {handle} name={user?.name ?? handle} avatarUrl={user?.avatar_url} size="sm" />
											@{handle}
										</a>
									</Table.Cell>
									<Table.Cell class="px-4 text-foreground">{user?.name ?? handle}</Table.Cell>
									<Table.Cell class="px-4 text-muted-foreground">{user?.title ?? ''}</Table.Cell>
									<Table.Cell class="px-4">
										<Badge variant="outline" class="bg-muted text-muted-foreground">
											{user?.status ?? 'departed'}
										</Badge>
									</Table.Cell>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</div>
			{/if}
		{/if}

		{#if teamServices.length > 0}
			<div>
				<h2 class="text-xs font-medium uppercase tracking-wide text-muted-foreground mb-3">Services</h2>
				<div class="grid gap-3 sm:grid-cols-2">
					{#each teamServices as svc (svc.slug)}
						<ServiceCardMini service={svc} />
					{/each}
				</div>
			</div>
		{/if}

		{#if !$assignmentsLoading}

			{#if openItems.length > 0}
				<Card.Root>
					<Card.Header>
						<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Open Items</Card.Title>
					</Card.Header>
					<Card.Content class="p-0">
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
												<StatusBadge status={a.status} docType={a.doc_type} />
											{/if}
										</Table.Cell>
										<Table.Cell class="px-4 text-muted-foreground whitespace-nowrap">{a.due_date ?? ''}</Table.Cell>
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
		{/if}
	{/if}
</div>
