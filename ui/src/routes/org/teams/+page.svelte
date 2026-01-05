<script lang="ts">
	import { orgData, orgLoading } from '$lib/stores/org';
	import UsersIcon from '@lucide/svelte/icons/users';
	import Building2Icon from '@lucide/svelte/icons/building-2';
	import { isInactive } from '$lib/utils';

	const teams = $derived($orgData ? Object.entries($orgData.teams) : []);
</script>

<svelte:head>
	<title>Teams</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	<h1 class="text-2xl font-bold text-foreground mb-6">Teams</h1>

	{#if $orgLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if teams.length === 0}
		<div class="text-muted-foreground">No teams configured.</div>
	{:else}
		<div class="grid gap-4 sm:grid-cols-2">
			{#each teams as [id, team]}
				<a href="/org/teams/{id}" class="rounded-xl border bg-card p-5 shadow-sm transition-all hover:shadow-md {isInactive(team.status) ? 'opacity-60' : ''}">
					<div class="flex items-center gap-3">
						<UsersIcon class="size-5 text-muted-foreground shrink-0" />
						<div>
							<h2 class="text-lg font-semibold {isInactive(team.status) ? 'text-muted-foreground' : 'text-foreground'}">
								{team.name}
								{#if isInactive(team.status)}
									<span class="ml-1.5 inline-block rounded bg-muted px-1.5 py-0.5 text-[10px] font-medium uppercase text-muted-foreground align-middle">deprecated</span>
								{/if}
							</h2>
							{#if team.org && $orgData?.orgs[team.org]}
								<div class="mt-0.5 text-xs text-muted-foreground flex items-center gap-1">
									<Building2Icon class="size-3 shrink-0" />
									{$orgData.orgs[team.org].name}
								</div>
							{/if}
							<div class="mt-1 text-sm text-muted-foreground">{team.members.length} {team.members.length === 1 ? 'member' : 'members'}</div>
							{#if team.lead}
								<div class="mt-1 text-sm text-muted-foreground">Lead: @{team.lead}</div>
							{/if}
						</div>
					</div>
				</a>
			{/each}
		</div>
	{/if}
</div>
