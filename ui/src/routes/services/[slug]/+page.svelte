<script lang="ts">
	import { page } from '$app/state';
	import { allServices, servicesLoading, loadServices, serviceBySlug } from '$lib/stores/services';
	import { orgData } from '$lib/stores/org';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import HtmlContent from '$lib/components/HtmlContent.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import SourceFileLink from '$lib/components/SourceFileLink.svelte';
	import UserAvatar from '$lib/components/UserAvatar.svelte';
	import DeviconIcon from '$lib/components/DeviconIcon.svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { onMount } from 'svelte';

	onMount(() => loadServices());

	const slug = $derived(page.params.slug ?? '');
	const service = $derived(serviceBySlug($allServices, slug));
	const isTeamOwned = $derived(!!service?.owner_team);
	const team = $derived(isTeamOwned ? $orgData?.teams[service!.owner_team!] : undefined);
	const user = $derived(!isTeamOwned && service?.owner ? $orgData?.users[service.owner] : undefined);

	function formatLoC(n: number): string {
		if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
		return n.toString();
	}

	function formatAge(iso: string): string {
		const created = new Date(iso);
		const now = new Date();
		const months =
			(now.getFullYear() - created.getFullYear()) * 12 + (now.getMonth() - created.getMonth());
		if (months < 1) return 'new';
		if (months < 12) return `${months}mo`;
		const years = Math.floor(months / 12);
		const rem = months % 12;
		return rem > 0 ? `${years}y ${rem}mo` : `${years}y`;
	}

	const statusColors: Record<string, string> = {
		live: 'border-t-emerald-500',
		beta: 'border-t-amber-500',
		planned: 'border-t-blue-500',
		sunset: 'border-t-red-500',
		deprecated: 'border-t-red-500'
	};

	const accent = $derived(
		service ? (statusColors[service.status.toLowerCase()] ?? 'border-t-gray-400') : ''
	);

	/** Color for language bar segments */
	const langColors: Record<string, string> = {
		rust: '#dea584',
		typescript: '#3178c6',
		javascript: '#f1e05a',
		python: '#3572A5',
		ruby: '#701516',
		go: '#00ADD8',
		java: '#b07219',
		php: '#4F5D95',
		swift: '#F05138',
		kotlin: '#A97BFF',
		'c#': '#178600',
		c: '#555555',
		'c++': '#f34b7d',
		html: '#e34c26',
		css: '#563d7c',
		shell: '#89e051',
		scss: '#c6538c',
		svelte: '#ff3e00'
	};

	function langColor(name: string): string {
		return langColors[name.toLowerCase()] ?? '#6b7280';
	}
</script>

<svelte:head>
	<title>{service ? service.name : 'Service'}</title>
</svelte:head>

<div class="mx-auto max-w-5xl">
	{#if $servicesLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else if !service}
		<div class="text-muted-foreground">Service not found: {slug}</div>
	{:else}
		<div class="flex items-center justify-between">
			<Breadcrumb
				crumbs={[{ label: 'Services', href: '/' }, { label: service.name }]}
			/>
			{#if service.readme_path}
				<SourceFileLink path={service.readme_path} />
			{/if}
		</div>

		<div class="xl:grid xl:grid-cols-[1fr_280px] xl:gap-8">
			<!-- Main article card -->
			<article class="rounded-xl border border-t-4 {accent} bg-card text-card-foreground shadow-sm">
				<!-- Header -->
				<div class="px-6 pt-6 pb-4">
					<div class="flex flex-wrap items-center gap-2 mb-3">
						<StatusBadge status={service.status} />
					</div>

					<h1 class="text-2xl font-bold tracking-tight text-foreground mb-2">
						{service.name}
					</h1>

					{#if service.description}
						<p class="text-sm text-muted-foreground mb-3">{service.description}</p>
					{/if}

					<!-- Owner row -->
					{#if service.owner && service.owner !== 'Unknown'}
						<div class="flex items-center gap-1.5 text-sm text-muted-foreground">
							<span>Owner:</span>
							{#if isTeamOwned}
								<a
									href="/org/teams/{service.owner_team}"
									class="inline-flex items-center gap-1 no-underline hover:text-foreground transition-colors font-medium text-foreground"
								>
									{team?.name ?? service.owner_team}
								</a>
							{:else}
								<a
									href="/org/users/{service.owner}"
									class="inline-flex items-center gap-1 no-underline hover:text-foreground transition-colors"
								>
									<UserAvatar
										handle={service.owner}
										name={user?.name ?? service.owner}
										avatarUrl={user?.avatar_url}
										size="sm"
									/>
									<span>{user?.name ?? `@${service.owner}`}</span>
								</a>
							{/if}
						</div>
					{/if}
				</div>

				<Separator />

				<!-- Body content -->
				{#if service.body_html}
					<div
						class="prose prose-slate dark:prose-invert max-w-none prose-headings:font-semibold prose-a:text-primary min-w-0 px-6 py-6"
					>
						<HtmlContent html={service.body_html} />
					</div>
				{:else}
					<div class="px-6 py-6 text-sm text-muted-foreground">
						No additional documentation available.
					</div>
				{/if}
			</article>

			<!-- Sidebar -->
			<aside class="mt-8 xl:mt-0 space-y-4 xl:sticky xl:top-20 xl:self-start">
				<!-- Tech Stack -->
				{#if service.languages?.length > 0}
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title
								class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
								>Tech Stack</Card.Title
							>
						</Card.Header>
						<Card.Content class="space-y-3">
							<!-- Language bar -->
							<div class="flex h-2 w-full overflow-hidden rounded-full bg-muted">
								{#each service.languages as lang (lang.name)}
									<div
										style="width: {lang.percentage}%; background: {langColor(lang.name)};"
										title="{lang.name} ({lang.percentage.toFixed(1)}%)"
									></div>
								{/each}
							</div>
							<!-- Language legend -->
							<div class="flex flex-wrap gap-x-3 gap-y-1 text-xs">
								{#each service.languages as lang (lang.name)}
									<span class="inline-flex items-center gap-1">
										<span
											class="inline-block h-2 w-2 rounded-full"
											style="background: {langColor(lang.name)};"
										></span>
										{lang.name}
										<span class="text-muted-foreground"
											>{lang.percentage.toFixed(1)}%</span
										>
									</span>
								{/each}
							</div>

							<!-- Frameworks -->
							{#if service.frameworks?.length > 0}
								<div class="flex flex-wrap gap-1.5 pt-1">
									{#each service.frameworks as fw (fw)}
										{@const version = service.framework_versions?.find(
											(fv) => fv[0] === fw
										)}
										<span
											class="inline-flex items-center gap-1 rounded-full bg-muted px-2.5 py-1 text-xs"
										>
											<DeviconIcon name={fw} size="sm" />
											{fw}
											{#if version}
												<span class="text-muted-foreground">{version[1]}</span>
											{/if}
										</span>
									{/each}
								</div>
							{/if}
						</Card.Content>
					</Card.Root>
				{/if}

				<!-- Infrastructure -->
				{#if service.deployment_platform || service.database}
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title
								class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
								>Infrastructure</Card.Title
							>
						</Card.Header>
						<Card.Content class="space-y-2 text-sm">
							{#if service.deployment_platform}
								<div class="flex items-center gap-2">
									<DeviconIcon name={service.deployment_platform} size="sm" />
									<span class="text-muted-foreground">Deploy:</span>
									<span>{service.deployment_platform}</span>
								</div>
							{/if}
							{#if service.database}
								<div class="flex items-center gap-2">
									<DeviconIcon name={service.database} size="sm" />
									<span class="text-muted-foreground">Database:</span>
									<span>{service.database}</span>
								</div>
							{/if}
						</Card.Content>
					</Card.Root>
				{/if}

				<!-- Stats -->
				{#if service.lines_of_code || service.dependencies_count || service.repo_size || service.language_version || service.created_at}
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title
								class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
								>Stats</Card.Title
							>
						</Card.Header>
						<Card.Content>
							<dl class="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
								{#if service.lines_of_code}
									<dt class="text-muted-foreground">Lines of code</dt>
									<dd class="text-right font-medium">
										{formatLoC(service.lines_of_code)}
									</dd>
								{/if}
								{#if service.dependencies_count}
									<dt class="text-muted-foreground">Dependencies</dt>
									<dd class="text-right font-medium">
										{service.dependencies_count}
									</dd>
								{/if}
								{#if service.repo_size}
									<dt class="text-muted-foreground">Repo size</dt>
									<dd class="text-right font-medium">{service.repo_size}</dd>
								{/if}
								{#if service.language_version}
									<dt class="text-muted-foreground">Language</dt>
									<dd class="text-right font-medium">
										{service.language_version}
									</dd>
								{/if}
								{#if service.created_at}
									<dt class="text-muted-foreground">Age</dt>
									<dd class="text-right font-medium">
										{formatAge(service.created_at)}
									</dd>
								{/if}
							</dl>
						</Card.Content>
					</Card.Root>
				{/if}
			</aside>
		</div>
	{/if}
</div>
