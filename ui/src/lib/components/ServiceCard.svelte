<script lang="ts">
	import StatusBadge from './StatusBadge.svelte';
	import UserAvatar from './UserAvatar.svelte';
	import DeviconIcon from './DeviconIcon.svelte';
	import { orgData } from '$lib/stores/org';
	import type { ServiceEntry, SchemaEnumValue } from '$lib/types';

	interface Props {
		service: ServiceEntry;
	}

	let { service }: Props = $props();

	const serviceStatuses: SchemaEnumValue[] = [
		{ name: 'planned', description: 'In planning phase', transitions: ['beta', 'live'] },
		{ name: 'beta', description: 'In beta testing', transitions: ['live', 'sunset'] },
		{ name: 'live', description: 'Running in production', transitions: ['sunset', 'deprecated'] },
		{ name: 'sunset', description: 'Being phased out', transitions: ['deprecated'] },
		{ name: 'deprecated', description: 'No longer maintained' }
	];

	const statusColors: Record<string, string> = {
		live: 'border-l-emerald-500',
		beta: 'border-l-amber-500',
		planned: 'border-l-blue-500',
		sunset: 'border-l-red-500',
		deprecated: 'border-l-red-500'
	};

	const neonGlow: Record<string, string> = {
		live: '0 0 8px rgba(16,185,129,0.4)',
		beta: '0 0 8px rgba(245,158,11,0.4)',
		planned: '0 0 8px rgba(59,130,246,0.4)',
		sunset: '0 0 8px rgba(239,68,68,0.4)',
		deprecated: '0 0 8px rgba(239,68,68,0.4)'
	};

	/** Map language names from onefetch to friendly display names */
	const languageDisplayNames: Record<string, string> = {
		Go: 'Golang',
		JavaScript: 'JavaScript',
		TypeScript: 'TypeScript',
		Python: 'Python',
		Ruby: 'Ruby',
		Rust: 'Rust',
		Elixir: 'Elixir',
		PHP: 'PHP',
		Java: 'Java',
		Kotlin: 'Kotlin',
		Swift: 'Swift',
		'C#': 'C#',
		'C++': 'C++',
		C: 'C',
		Dart: 'Dart',
		Scala: 'Scala',
		Haskell: 'Haskell',
		Zig: 'Zig',
		Lua: 'Lua',
		Perl: 'Perl',
		Shell: 'Shell',
		HTML: 'HTML',
		CSS: 'CSS',
		SCSS: 'SCSS',
	};

	const borderColor = $derived(statusColors[service.status.toLowerCase()] ?? 'border-l-gray-400');
	const glow = $derived(neonGlow[service.status.toLowerCase()] ?? '0 0 8px rgba(156,163,175,0.4)');
	const isTeamOwned = $derived(!!service.owner_team);
	const team = $derived(isTeamOwned ? $orgData?.teams[service.owner_team!] : undefined);
	const user = $derived(!isTeamOwned && service.owner ? $orgData?.users[service.owner] : undefined);
	const pills = $derived([
		service.primary_language,
		...(service.frameworks ?? []),
		...(service.deployment_platform ? [service.deployment_platform] : []),
		...(service.database ? [service.database] : [])
	]);

	function displayLang(name: string): string {
		return languageDisplayNames[name] ?? name;
	}

	function formatLoC(n: number): string {
		if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
		return n.toString();
	}

	function formatAge(iso: string): string {
		const created = new Date(iso);
		const now = new Date();
		const months = (now.getFullYear() - created.getFullYear()) * 12 + (now.getMonth() - created.getMonth());
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
</script>

<a
	href="/software/{service.kind === 'app' ? 'apps' : service.kind === 'infra' ? 'infra' : 'services'}/{service.slug}"
	class="service-card block rounded-xl border border-l-4 {borderColor} bg-card p-4 shadow-sm transition-all hover:shadow-md cursor-pointer no-underline text-inherit"
	style="--neon-glow: {glow}"
>
	<!-- Header -->
	<div class="flex items-start justify-between gap-2">
		<div class="min-w-0 flex-1">
			<h3 class="text-sm font-semibold text-foreground">{service.name}</h3>
			{#if service.description_html}
				<div
					class="mt-0.5 text-xs text-muted-foreground line-clamp-2 [&_code]:rounded [&_code]:bg-muted [&_code]:px-0.5 [&_code]:font-mono [&_code]:text-[0.7rem]"
				>
					{@html service.description_html}
				</div>
			{:else if service.description}
				<p class="mt-0.5 text-xs text-muted-foreground line-clamp-2">{service.description}</p>
			{/if}
		</div>
		<StatusBadge status={service.status} overrideStatuses={serviceStatuses} />
	</div>

	<!-- Technology pills: language, frameworks, infra, database — all in one row -->
	<div class="mt-3 flex flex-wrap gap-1">
		{#each pills as tech (tech)}
			<span class="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
				<DeviconIcon name={tech} size="sm" />
				{tech === service.primary_language ? displayLang(tech) : tech}
			</span>
		{/each}
	</div>

	<!-- Engineering practices badges -->
	<div class="mt-2 flex flex-wrap gap-1">
		{#if service.eol_warnings?.length}
			<span class="inline-flex items-center rounded-full bg-red-100 dark:bg-red-900/30 px-2 py-0.5 text-xs font-medium text-red-700 dark:text-red-400">
				EOL
			</span>
		{/if}
		{#if service.has_linter}
			<span class="inline-flex items-center gap-0.5 rounded-full bg-emerald-100 dark:bg-emerald-900/30 px-2 py-0.5 text-xs text-emerald-700 dark:text-emerald-400">
				{service.linter_tool ?? 'Linter'} ✓
			</span>
		{:else}
			<span class="inline-flex items-center gap-0.5 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
				No Linter
			</span>
		{/if}
		{#if service.has_tests}
			<span class="inline-flex items-center gap-0.5 rounded-full bg-emerald-100 dark:bg-emerald-900/30 px-2 py-0.5 text-xs text-emerald-700 dark:text-emerald-400">
				{service.test_framework ?? 'Tests'} ✓
			</span>
		{:else}
			<span class="inline-flex items-center gap-0.5 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
				No Tests
			</span>
		{/if}
	</div>

	<!-- Footer -->
	<div class="mt-3 flex items-center gap-3 text-xs text-muted-foreground border-t border-border pt-2">
		{#if isTeamOwned}
			<span class="inline-flex items-center gap-1 text-foreground font-medium">{team?.name ?? service.owner_team}</span>
		{:else if service.owner && service.owner !== 'Unknown'}
			<UserAvatar handle={service.owner} name={user?.name ?? service.owner} avatarUrl={user?.avatar_url} size="sm" />
		{/if}
		{#if service.lines_of_code}
			<span>{formatLoC(service.lines_of_code)} LoC</span>
		{/if}
		{#if service.dependencies_count}
			<span>{service.dependencies_count} deps</span>
		{/if}
		{#if service.commit_count}
			<span>{service.commit_count} commits</span>
		{/if}
		{#if service.last_commit_at}
			<span title="Last commit">{formatRelative(service.last_commit_at)}</span>
		{:else if service.created_at}
			<span>{formatAge(service.created_at)}</span>
		{/if}
	</div>
</a>

<style>
	:global(.dark) .service-card:hover {
		box-shadow: var(--neon-glow);
	}
</style>
