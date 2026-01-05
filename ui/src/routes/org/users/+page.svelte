<script lang="ts">
	import { orgData, orgLoading } from '$lib/stores/org';
	import UserAvatar from '$lib/components/UserAvatar.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import { isInactive } from '$lib/utils';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import ArrowUpDownIcon from '@lucide/svelte/icons/arrow-up-down';
	import UserXIcon from '@lucide/svelte/icons/user-x';
	import BotIcon from '@lucide/svelte/icons/bot';

	type SortCol = 'handle' | 'name' | 'title' | 'kind' | 'tenure' | 'status';
	type SortDir = 'asc' | 'desc';

	let sortCol = $state<SortCol>('name');
	let sortDir = $state<SortDir>('asc');

	function toggleSort(col: SortCol) {
		if (sortCol === col) {
			sortDir = sortDir === 'asc' ? 'desc' : 'asc';
		} else {
			sortCol = col;
			sortDir = 'asc';
		}
	}

	function sortUsers(list: [string, any][]): [string, any][] {
		return [...list].sort((a, b) => {
			const [hA, uA] = a;
			const [hB, uB] = b;
			let cmp = 0;
			switch (sortCol) {
				case 'handle':
					cmp = hA.localeCompare(hB);
					break;
				case 'name':
					cmp = (uA.name ?? '').localeCompare(uB.name ?? '');
					break;
				case 'title':
					cmp = (uA.title ?? '').localeCompare(uB.title ?? '');
					break;
				case 'kind':
					cmp = (uA.kind ?? '').localeCompare(uB.kind ?? '');
					break;
				case 'tenure':
					cmp = (uA.started ?? '').localeCompare(uB.started ?? '');
					break;
				case 'status':
					cmp = (uA.status ?? '').localeCompare(uB.status ?? '');
					break;
			}
			return sortDir === 'asc' ? cmp : -cmp;
		});
	}

	const users = $derived($orgData ? Object.entries($orgData.users) : []);
	const activeUsers = $derived(sortUsers(users.filter(([, u]) => u.status !== 'departed' && u.kind !== 'ai')));
	const aiUsers = $derived(users.filter(([, u]) => u.kind === 'ai'));
	const departedUsers = $derived(sortUsers(users.filter(([, u]) => u.status === 'departed' && u.kind !== 'ai')));
	let showDeparted = $state(false);

	function formatTenure(started: string | undefined): string {
		if (!started) return '';
		const start = new Date(started);
		const now = new Date();
		const diffMs = now.getTime() - start.getTime();
		if (diffMs < 0) return '';
		const totalDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
		const years = Math.floor(totalDays / 365.25);
		const remainingDays = totalDays - Math.floor(years * 365.25);
		const months = Math.floor(remainingDays / 30.44);
		const weeks = Math.floor(remainingDays / 7);
		if (years >= 1) {
			return months > 0 ? `${years}y ${months}m` : `${years}y`;
		}
		if (months >= 1) return `${months}m`;
		return `${weeks}w`;
	}
</script>

<svelte:head>
	<title>People</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	<h1 class="text-2xl font-bold text-foreground mb-6">People</h1>

	{#if $orgLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if users.length === 0}
		<div class="text-muted-foreground">No users configured.</div>
	{:else}
		<div class="rounded-lg border bg-card">
			<Table.Root>
				<Table.Header>
					<Table.Row>
						{#each [['handle','Handle'],['name','Name'],['title','Title'],['kind','Kind']] as [col, label]}
							<Table.Head class="px-4">
								<button type="button" class="inline-flex items-center gap-1 hover:text-foreground transition-colors" onclick={() => toggleSort(col as SortCol)}>
									{label}
									{#if sortCol === col}
										{#if sortDir === 'asc'}<ArrowUpIcon class="size-3" />{:else}<ArrowDownIcon class="size-3" />{/if}
									{:else}
										<ArrowUpDownIcon class="size-3 opacity-30" />
									{/if}
								</button>
							</Table.Head>
						{/each}
						<Table.Head class="px-4">Teams</Table.Head>
						{#each [['tenure','Tenure'],['status','Status']] as [col, label]}
							<Table.Head class="px-4">
								<button type="button" class="inline-flex items-center gap-1 hover:text-foreground transition-colors" onclick={() => toggleSort(col as SortCol)}>
									{label}
									{#if sortCol === col}
										{#if sortDir === 'asc'}<ArrowUpIcon class="size-3" />{:else}<ArrowDownIcon class="size-3" />{/if}
									{:else}
										<ArrowUpDownIcon class="size-3 opacity-30" />
									{/if}
								</button>
							</Table.Head>
						{/each}
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each activeUsers as [handle, user]}
						<Table.Row class={isInactive(user.status) ? 'opacity-60' : ''}>
							<Table.Cell class="px-4">
								<a href="/org/users/{handle}" class="inline-flex items-center gap-2 text-primary hover:underline font-medium">
									<UserAvatar {handle} name={user.name} avatarUrl={user.avatar_url} size="sm" />
									@{handle}
								</a>
							</Table.Cell>
							<Table.Cell class="px-4 text-foreground">{user.name}</Table.Cell>
							<Table.Cell class="px-4 text-muted-foreground">{user.title ?? ''}</Table.Cell>
							<Table.Cell class="px-4">
								{#if user.kind && user.kind !== 'internal'}
									<Badge variant="outline" class={user.kind === 'ai' ? 'bg-violet-100 text-violet-800 border-violet-200' : 'bg-blue-100 text-blue-800 border-blue-200'}>
										{user.kind}
									</Badge>
								{:else}
									<Badge variant="outline" class="bg-emerald-100 text-emerald-800 border-emerald-200">internal</Badge>
								{/if}
							</Table.Cell>
							<Table.Cell class="px-4">
								<div class="flex flex-wrap gap-1">
									{#each user.teams as teamId}
										<Badge variant="secondary" href="/org/teams/{teamId}">{teamId}</Badge>
									{/each}
								</div>
							</Table.Cell>
							<Table.Cell class="px-4 text-xs text-muted-foreground whitespace-nowrap">{formatTenure(user.started)}</Table.Cell>
							<Table.Cell class="px-4">
								<StatusBadge status={user.status} />
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		</div>

		{#if departedUsers.length > 0}
			<div class="mt-4">
				<button
					type="button"
					class="w-full text-left"
					onclick={() => showDeparted = !showDeparted}
				>
					<Alert.Root class="border-gray-200 bg-gray-50 text-gray-600 dark:border-gray-700 dark:bg-gray-900/30 dark:text-gray-400 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-900/50 transition-colors">
						<UserXIcon class="text-gray-500 dark:text-gray-500" />
						<Alert.Title class="font-medium">{departedUsers.length} departed {departedUsers.length === 1 ? 'person' : 'people'}</Alert.Title>
						<Alert.Description class="text-gray-500/70 dark:text-gray-500/70 flex items-center gap-1">
							{showDeparted ? 'Click to hide' : 'Click to show'} departed members
							<ChevronDownIcon class="size-3.5 transition-transform {showDeparted ? 'rotate-180' : ''}" />
						</Alert.Description>
					</Alert.Root>
				</button>
			</div>

			{#if showDeparted}
				<div class="mt-2 rounded-lg border bg-card opacity-60">
					<Table.Root>
						<Table.Header>
							<Table.Row>
								{#each [['handle','Handle'],['name','Name'],['title','Title'],['kind','Kind']] as [col, label]}
									<Table.Head class="px-4">
										<button type="button" class="inline-flex items-center gap-1 hover:text-foreground transition-colors" onclick={() => toggleSort(col as SortCol)}>
											{label}
											{#if sortCol === col}
												{#if sortDir === 'asc'}<ArrowUpIcon class="size-3" />{:else}<ArrowDownIcon class="size-3" />{/if}
											{:else}
												<ArrowUpDownIcon class="size-3 opacity-30" />
											{/if}
										</button>
									</Table.Head>
								{/each}
								<Table.Head class="px-4">Teams</Table.Head>
								{#each [['tenure','Tenure'],['status','Status']] as [col, label]}
									<Table.Head class="px-4">
										<button type="button" class="inline-flex items-center gap-1 hover:text-foreground transition-colors" onclick={() => toggleSort(col as SortCol)}>
											{label}
											{#if sortCol === col}
												{#if sortDir === 'asc'}<ArrowUpIcon class="size-3" />{:else}<ArrowDownIcon class="size-3" />{/if}
											{:else}
												<ArrowUpDownIcon class="size-3 opacity-30" />
											{/if}
										</button>
									</Table.Head>
								{/each}
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each departedUsers as [handle, user]}
								<Table.Row>
									<Table.Cell class="px-4">
										<a href="/org/users/{handle}" class="inline-flex items-center gap-2 text-primary hover:underline font-medium">
											<UserAvatar {handle} name={user.name} avatarUrl={user.avatar_url} size="sm" />
											@{handle}
										</a>
									</Table.Cell>
									<Table.Cell class="px-4 text-foreground">{user.name}</Table.Cell>
									<Table.Cell class="px-4 text-muted-foreground">{user.title ?? ''}</Table.Cell>
									<Table.Cell class="px-4">
										{#if user.kind && user.kind !== 'internal'}
											<Badge variant="outline" class={user.kind === 'ai' ? 'bg-violet-100 text-violet-800 border-violet-200' : 'bg-blue-100 text-blue-800 border-blue-200'}>
												{user.kind}
											</Badge>
										{:else}
											<Badge variant="outline" class="bg-emerald-100 text-emerald-800 border-emerald-200">internal</Badge>
										{/if}
									</Table.Cell>
									<Table.Cell class="px-4">
										<div class="flex flex-wrap gap-1">
											{#each user.teams as teamId}
												<Badge variant="secondary" href="/org/teams/{teamId}">{teamId}</Badge>
											{/each}
										</div>
									</Table.Cell>
									<Table.Cell class="px-4 text-xs text-muted-foreground whitespace-nowrap">{formatTenure(user.started)}</Table.Cell>
									<Table.Cell class="px-4">
										<StatusBadge status={user.status} />
									</Table.Cell>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>
				</div>
			{/if}
		{/if}

		{#if aiUsers.length > 0}
			<h2 class="text-lg font-semibold text-foreground mt-8 mb-3 flex items-center gap-2">
				<BotIcon class="size-5 text-violet-600" />
				AI Assistants
			</h2>
			<div class="rounded-lg border bg-card">
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="px-4">Handle</Table.Head>
							<Table.Head class="px-4">Name</Table.Head>
							<Table.Head class="px-4">Title</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each aiUsers as [handle, user]}
							<Table.Row>
								<Table.Cell class="px-4">
									<a href="/org/users/{handle}" class="inline-flex items-center gap-2 text-primary hover:underline font-medium">
										<UserAvatar {handle} name={user.name} avatarUrl={user.avatar_url} size="sm" />
										@{handle}
									</a>
								</Table.Cell>
								<Table.Cell class="px-4 text-foreground">{user.name}</Table.Cell>
								<Table.Cell class="px-4 text-muted-foreground">{user.title ?? ''}</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</div>
		{/if}
	{/if}
</div>
