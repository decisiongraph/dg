<script lang="ts">
	import { page } from '$app/state';
	import { orgData, orgLoading } from '$lib/stores/org';
	import SourceFileLink from '$lib/components/SourceFileLink.svelte';

	const orgId = $derived(page.params.id ?? '');
	const org = $derived($orgData?.orgs[orgId]);
</script>

<svelte:head>
	<title>{org?.name ?? orgId ?? 'Organization'}</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	{#if $orgLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if !org}
		<div class="text-muted-foreground">Organization not found: {orgId}</div>
	{:else}
		<div class="flex items-center justify-between mb-6">
			<h1 class="text-2xl font-bold text-foreground">{org.name}</h1>
			<SourceFileLink path=".dg/org.kdl" />
		</div>

		{#if org.parent}
			<div class="mb-4 text-sm text-muted-foreground">
				Parent: <a href="/org/{org.parent}" class="text-primary hover:underline">{$orgData?.orgs[org.parent]?.name ?? org.parent}</a>
			</div>
		{/if}

		{#if org.children.length > 0}
			<section class="mb-6">
				<h2 class="text-lg font-semibold text-foreground mb-3">Subsidiaries</h2>
				<div class="grid gap-3 sm:grid-cols-2">
					{#each org.children as childId}
						{@const child = $orgData?.orgs[childId]}
						<a href="/org/{childId}" class="rounded-lg border border-border bg-card p-4 hover:shadow-md transition-shadow">
							<div class="font-semibold text-foreground">{child?.name ?? childId}</div>
						</a>
					{/each}
				</div>
			</section>
		{/if}

		<!-- Teams in this org -->
		{@const orgTeams = Object.entries($orgData?.teams ?? {}).filter(([_, t]) => t.org === orgId).sort(([, a], [, b]) => {
			const aDeprecated = a.status === 'deprecated' ? 1 : 0;
			const bDeprecated = b.status === 'deprecated' ? 1 : 0;
			return aDeprecated - bDeprecated;
		})}
		{#if orgTeams.length > 0}
			<section>
				<h2 class="text-lg font-semibold text-foreground mb-3">Teams</h2>
				<div class="grid gap-3 sm:grid-cols-2">
					{#each orgTeams as [teamId, team]}
						<a href="/org/teams/{teamId}" class="rounded-lg border border-border bg-card p-4 hover:shadow-md transition-shadow {team.status === 'deprecated' ? 'opacity-50' : ''}">
							<div class="font-semibold text-foreground">{team.name}</div>
							<div class="text-xs text-muted-foreground">{team.members.length} members</div>
						</a>
					{/each}
				</div>
			</section>
		{/if}
	{/if}
</div>
