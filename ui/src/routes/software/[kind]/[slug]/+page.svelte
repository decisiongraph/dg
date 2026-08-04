<script lang="ts">
	import { page } from '$app/state';
	import { allServices, servicesLoading, loadServices, serviceBySlug } from '$lib/stores/services';
	import { orgData } from '$lib/stores/org';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import HtmlContent from '$lib/components/HtmlContent.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import UserAvatar from '$lib/components/UserAvatar.svelte';
	import DeviconIcon from '$lib/components/DeviconIcon.svelte';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { onMount } from 'svelte';

	onMount(() => loadServices());

	const kindSlug = $derived(page.params.kind ?? '');
	const slug = $derived(page.params.slug ?? '');
	const service = $derived(serviceBySlug($allServices, slug));
	const isTeamOwned = $derived(!!service?.owner_team);
	const team = $derived(isTeamOwned ? $orgData?.teams[service!.owner_team!] : undefined);
	const user = $derived(!isTeamOwned && service?.owner ? $orgData?.users[service.owner] : undefined);

	const kindDisplayMap: Record<string, string> = {
		services: 'Services',
		apps: 'Apps',
		infra: 'Infra',
	};

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

	function formatRelative(iso: string): string {
		const date = new Date(iso);
		const now = new Date();
		const days = Math.floor((now.getTime() - date.getTime()) / 86400000);
		if (days === 0) return 'today';
		if (days === 1) return '1d ago';
		if (days < 7) return `${days}d ago`;
		if (days < 30) return `${Math.floor(days / 7)}w ago`;
		if (days < 365) return `${Math.floor(days / 30)}mo ago`;
		return `${Math.floor(days / 365)}y ago`;
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
		<div class="text-muted-foreground">Not found: {slug}</div>
	{:else}
		<Breadcrumb
			crumbs={[
				{ label: kindDisplayMap[kindSlug] ?? 'Services', href: `/software/${kindSlug}` },
				{ label: service.name }
			]}
		/>

		<div class="xl:grid xl:grid-cols-[1fr_280px] xl:gap-8">
			<article class="rounded-xl border border-t-4 {accent} bg-card text-card-foreground shadow-sm min-w-0">
				<div class="px-6 pt-6 pb-4">
					<div class="flex flex-wrap items-center gap-2 mb-3">
						<StatusBadge status={service.status} />
					</div>

					<h1 class="text-2xl font-bold tracking-tight text-foreground mb-2">
						{service.name}
					</h1>

					{#if service.description_html}
						<div
							class="text-sm text-muted-foreground mb-3 [&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-xs [&_a]:text-primary"
						>
							{@html service.description_html}
						</div>
					{:else if service.description}
						<p class="text-sm text-muted-foreground mb-3">{service.description}</p>
					{/if}

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

					{#if service.source_url}
						<a
							href={service.source_url}
							target="_blank"
							rel="noopener noreferrer"
							class="inline-flex items-center gap-1.5 mt-3 text-sm text-muted-foreground hover:text-foreground transition-colors no-underline"
						>
							<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"/><path d="M9 18c-4.51 2-5-2-7-2"/></svg>
							View Source Repository
							<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
						</a>
					{/if}
				</div>

				{#if service.eol_warnings?.length}
					<div class="mx-6 mb-4 rounded-lg border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950/30 p-4">
						<h4 class="text-sm font-medium text-red-700 dark:text-red-400">End-of-Life Versions</h4>
						<ul class="mt-2 text-sm text-red-600 dark:text-red-300 space-y-1">
							{#each service.eol_warnings as w}
								<li>{w.product} {w.version} — EOL{w.eol_date ? ` since ${w.eol_date}` : ''}</li>
							{/each}
						</ul>
					</div>
				{/if}

				<Separator />

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

			<aside class="mt-8 xl:mt-0 space-y-4 xl:sticky xl:top-20 xl:self-start">
				{#if service.languages?.length > 0}
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title
								class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
								>Tech Stack</Card.Title
							>
						</Card.Header>
						<Card.Content class="space-y-3">
							<div class="flex h-2 w-full overflow-hidden rounded-full bg-muted">
								{#each service.languages as lang (lang.name)}
									<div
										style="width: {lang.percentage}%; background: {langColor(lang.name)};"
										title="{lang.name} ({lang.percentage.toFixed(1)}%)"
									></div>
								{/each}
							</div>
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

							{#if service.frameworks?.length > 0 && service.kind !== 'infra'}
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

				<!-- Cloud platforms managed by this infra (from provider sources) -->
				{#if service.kind === 'infra' && service.frameworks?.length > 0}
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title
								class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
								>Managed Platforms</Card.Title
							>
						</Card.Header>
						<Card.Content>
							<div class="flex flex-wrap gap-1.5">
								{#each service.frameworks as platform (platform)}
									<span
										class="inline-flex items-center gap-1 rounded-full bg-muted px-2.5 py-1 text-xs"
									>
										<DeviconIcon name={platform} size="sm" />
										{platform}
									</span>
								{/each}
							</div>
						</Card.Content>
					</Card.Root>
				{/if}

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

				<!-- Engineering Practices -->
				<Card.Root>
					<Card.Header class="pb-2">
						<Card.Title
							class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
							>Practices</Card.Title
						>
					</Card.Header>
					<Card.Content class="space-y-2 text-sm">
						<div class="flex items-center gap-2">
							{#if service.has_linter}
								<span class="inline-flex items-center gap-1 rounded-full bg-emerald-100 dark:bg-emerald-900/30 px-2 py-0.5 text-xs text-emerald-700 dark:text-emerald-400">
									{service.linter_tool ?? 'Linter'} ✓
								</span>
							{:else}
								<span class="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
									No Linter
								</span>
							{/if}
							{#if service.has_tests}
								<span class="inline-flex items-center gap-1 rounded-full bg-emerald-100 dark:bg-emerald-900/30 px-2 py-0.5 text-xs text-emerald-700 dark:text-emerald-400">
									{service.test_framework ?? 'Tests'} ✓
								</span>
							{:else}
								<span class="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
									No Tests
								</span>
							{/if}
						</div>
					</Card.Content>
				</Card.Root>

			{#if service.lines_of_code || service.dependencies_count || service.repo_size || service.language_version || service.created_at || service.commit_count}
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
								{#if service.commit_count}
									<dt class="text-muted-foreground">Commits</dt>
									<dd class="text-right font-medium">
										{service.commit_count}
									</dd>
								{/if}
								{#if service.last_commit_at}
									<dt class="text-muted-foreground">Last commit</dt>
									<dd class="text-right font-medium">
										{formatRelative(service.last_commit_at)}
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
