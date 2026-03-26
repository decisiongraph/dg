<script lang="ts">
	import { page } from '$app/state';
	import { allDocs, docsLoading, docTypes } from '$lib/stores/docs';
	import { orgData } from '$lib/stores/org';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import HtmlContent from '$lib/components/HtmlContent.svelte';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';
	import UserAvatar from '$lib/components/UserAvatar.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import DocRefLink from '$lib/components/DocRefLink.svelte';
	import SourceFileLink from '$lib/components/SourceFileLink.svelte';
	import { schemaData } from '$lib/stores/schema';
	import { codeRefsData, loadCodeRefs, codeRefsForDoc } from '$lib/stores/code-refs';
	import {
		OUTGOING_LABELS,
		INCOMING_LABELS,
		SIDEBAR_SECTIONS,
		type RelationCategory
	} from '$lib/config/relations';

	loadCodeRefs();

	interface SidebarItem {
		id: string;
		title: string;
		label: string;
		relation: string;
	}

	const docId = $derived(page.params.id?.toUpperCase() ?? '');
	const doc = $derived($allDocs.find((d) => d.id.toLowerCase() === page.params.id?.toLowerCase()));
	const typeSlug = $derived(page.params.type ?? '');
	const typeDisplay = $derived(
		Object.values($docTypes).find((t) => t.folder === typeSlug)?.display ?? typeSlug
	);

	/** Group outgoing links + incoming backlinks into category buckets */
	const sidebarSections = $derived.by(() => {
		if (!doc) return new Map<RelationCategory, SidebarItem[]>();
		const buckets = new Map<RelationCategory, SidebarItem[]>();
		const seen = new Set<string>();

		// Outgoing links from frontmatter
		for (const [rel, refs] of Object.entries(doc.links)) {
			if (!refs?.length) continue;
			const cfg = OUTGOING_LABELS[rel];
			if (!cfg) continue;
			for (const ref of refs) {
				const key = `${cfg.category}:${ref}:${rel}`;
				if (seen.has(key)) continue;
				seen.add(key);
				const refDoc = $allDocs.find((d) => d.id.toLowerCase() === ref.toLowerCase());
				const items = buckets.get(cfg.category) ?? [];
				items.push({ id: ref, title: refDoc?.title ?? ref, label: cfg.label, relation: rel });
				buckets.set(cfg.category, items);
			}
		}

		// Incoming backlinks
		for (const bl of doc.backlinks) {
			const cfg = INCOMING_LABELS[bl.relation];
			if (!cfg) continue;
			const key = `${cfg.category}:${bl.id}:${bl.relation}`;
			if (seen.has(key)) continue;
			seen.add(key);
			const items = buckets.get(cfg.category) ?? [];
			items.push({ id: bl.id, title: bl.title, label: cfg.label, relation: bl.relation });
			buckets.set(cfg.category, items);
		}

		return buckets;
	});

	/** People entries: { role, handles[] } sorted with author/owner first */
	const peopleEntries = $derived.by(() => {
		if (!doc?.people) return [];
		const primary = ['author', 'owner', 'commander'];
		return Object.entries(doc.people)
			.filter(([, handles]) => handles.length > 0)
			.sort(([a], [b]) => {
				const ai = primary.indexOf(a);
				const bi = primary.indexOf(b);
				if (ai !== -1 && bi !== -1) return ai - bi;
				if (ai !== -1) return -1;
				if (bi !== -1) return 1;
				return a.localeCompare(b);
			});
	});

	function resolveUser(handle: string) {
		return $orgData?.users[handle];
	}

	/** Map git author name → org user handle (fuzzy match on name) */
	function resolveAuthorHandle(authorName: string): string | undefined {
		const users = $orgData?.users;
		if (!users) return undefined;
		const lower = authorName.toLowerCase();
		for (const [handle, user] of Object.entries(users)) {
			if (user.name?.toLowerCase() === lower) return handle;
		}
		return undefined;
	}

	const schemaType = $derived(doc ? $schemaData?.types[doc.type] : undefined);
	const typeDescription = $derived(schemaType?.description);

	/** Look up a field description from schema for tooltip */
	function fieldTooltip(fieldName: string, value?: string): string | undefined {
		if (!schemaType) return undefined;
		const field = schemaType.fields[fieldName];
		if (!field) return undefined;
		if (value && field.values.length > 0) {
			const v = field.values.find((ev) => ev.name === value.toLowerCase());
			if (v?.description) return v.description;
		}
		return field.description;
	}

	const typeAccent: Record<string, string> = {
		adr: 'border-t-blue-500',
		opp: 'border-t-emerald-500',
		pol: 'border-t-violet-500',
		inc: 'border-t-red-500',
		spec: 'border-t-amber-500'
	};

	const severityColor: Record<string, string> = {
		sev1: 'bg-red-100 text-red-800 border-red-200',
		sev2: 'bg-orange-100 text-orange-800 border-orange-200',
		sev3: 'bg-amber-100 text-amber-800 border-amber-200',
		sev4: 'bg-slate-100 text-slate-700 border-slate-200',
		critical: 'bg-red-100 text-red-800 border-red-200',
		high: 'bg-orange-100 text-orange-800 border-orange-200',
		medium: 'bg-amber-100 text-amber-800 border-amber-200',
		low: 'bg-slate-100 text-slate-700 border-slate-200'
	};

	const accent = $derived(doc ? (typeAccent[doc.type] ?? 'border-t-gray-400') : '');

	/** Fields to show as colored badges instead of plain text */
	const badgeFields = new Set(['effort', 'impact', 'priority']);

	const effortColor: Record<string, string> = {
		small: 'bg-emerald-100 text-emerald-800 border-emerald-200',
		medium: 'bg-amber-100 text-amber-800 border-amber-200',
		large: 'bg-red-100 text-red-800 border-red-200',
	};
	const impactColor: Record<string, string> = {
		low: 'bg-slate-100 text-slate-700 border-slate-200',
		medium: 'bg-amber-100 text-amber-800 border-amber-200',
		high: 'bg-emerald-100 text-emerald-800 border-emerald-200',
	};

	const priorityColor: Record<string, string> = {
		critical: 'bg-red-100 text-red-800 border-red-200',
		high: 'bg-orange-100 text-orange-800 border-orange-200',
		medium: 'bg-amber-100 text-amber-800 border-amber-200',
		low: 'bg-slate-100 text-slate-700 border-slate-200',
	};

	/** Badge meta items: priority, effort, impact */
	const badgeMeta = $derived.by(() => {
		if (!doc) return [];
		const items: { label: string; value: string; colorClass: string }[] = [];
		if (doc.priority) {
			items.push({ label: 'Priority', value: doc.priority, colorClass: priorityColor[doc.priority.toLowerCase()] ?? '' });
		}
		const meta = doc.meta ?? {};
		if (meta.effort) {
			items.push({ label: 'Effort', value: meta.effort, colorClass: effortColor[meta.effort.toLowerCase()] ?? '' });
		}
		if (meta.impact) {
			items.push({ label: 'Impact', value: meta.impact, colorClass: impactColor[meta.impact.toLowerCase()] ?? '' });
		}
		return items;
	});

	/** Metadata items to show below the title */
	const metaItems = $derived.by(() => {
		if (!doc) return [];
		const items: { label: string; value: string }[] = [];
		if (doc.date) items.push({ label: 'Date', value: doc.date });
		// Extra meta fields from frontmatter (excluding badge fields)
		for (const [key, value] of Object.entries(doc.meta ?? {})) {
			if (value && !badgeFields.has(key)) items.push({ label: key.replace(/_/g, ' '), value });
		}
		return items;
	});

	// ── Code references ──────────────────────────────────────────
	const docCodeRefs = $derived(doc ? codeRefsForDoc($codeRefsData, doc.id) : null);
	const hasCommitRefs = $derived(docCodeRefs != null && docCodeRefs.commits.length > 0);
	const hasCodeFileRefs = $derived(docCodeRefs != null && docCodeRefs.code.length > 0);

	/** URL prefixes for linking commits/files to GitHub/GitLab */
	const commitUrlPrefix = $derived($codeRefsData?.commit_url_prefix ?? '');
	const fileUrlPrefix = $derived($codeRefsData?.file_url_prefix ?? '');

	/**
	 * Group code refs by file, then merge overlapping/adjacent context windows
	 * into unified blocks separated by '...' gaps.
	 */
	const codeRefsByFile = $derived.by(() => {
		if (!docCodeRefs?.code.length) return [];

		// Group by file
		const map = new Map<string, import('$lib/types').CodeRef[]>();
		for (const ref of docCodeRefs.code) {
			const entries = map.get(ref.file) ?? [];
			entries.push(ref);
			map.set(ref.file, entries);
		}

		return [...map.entries()].map(([file, refs]): { file: string; blocks: any[]; baseFileUrl: string } => {
			// Use submodule file_url from any ref, else fall back to prefix
			const submoduleUrl = refs.find((r) => r.file_url)?.file_url;
			const baseFileUrl = submoduleUrl ?? (fileUrlPrefix ? `${fileUrlPrefix}${file}` : '');
			// Build a sorted list of all lines (with content) that should appear
			// Each ref contributes: context_before lines + match line + context_after lines
			type LineEntry = { lineNum: number; text: string; isMatch: boolean };
			const lineMap = new Map<number, LineEntry>();

			for (const ref of refs) {
				const before = ref.context_before ?? [];
				const after = ref.context_after ?? [];
				const startLine = ref.line - before.length;
				before.forEach((t, i) => {
					const n = startLine + i;
					if (!lineMap.has(n)) lineMap.set(n, { lineNum: n, text: t, isMatch: false });
				});
				lineMap.set(ref.line, { lineNum: ref.line, text: ref.text, isMatch: true });
				after.forEach((t, i) => {
					const n = ref.line + 1 + i;
					if (!lineMap.has(n)) lineMap.set(n, { lineNum: n, text: t, isMatch: false });
				});
			}

			// Sort by line number
			const sorted = [...lineMap.values()].sort((a, b) => a.lineNum - b.lineNum);

			// Split into blocks separated by gaps, inserting '...' markers
			type Block = { lineNum: number; text: string; isMatch: boolean } | { gap: true };
			const blocks: Block[] = [];
			for (let i = 0; i < sorted.length; i++) {
				if (i > 0 && sorted[i].lineNum > sorted[i - 1].lineNum + 1) {
					blocks.push({ gap: true });
				}
				blocks.push(sorted[i]);
			}

			return { file, blocks, baseFileUrl };
		});
	});

	/** Map file extension → highlight.js language name */
	function extToLang(file: string): string | undefined {
		const ext = file.split('.').pop()?.toLowerCase();
		if (!ext) return undefined;
		const m: Record<string, string> = {
			rs: 'rust', ts: 'typescript', js: 'javascript', jsx: 'javascript',
			tsx: 'typescript', py: 'python', go: 'go', rb: 'ruby',
			java: 'java', kt: 'kotlin', swift: 'swift', c: 'c',
			cpp: 'cpp', h: 'c', hpp: 'cpp', cs: 'csharp',
			yaml: 'yaml', yml: 'yaml', toml: 'ini', json: 'json',
			sh: 'bash', bash: 'bash', zsh: 'bash', svelte: 'xml',
			html: 'xml', css: 'css', scss: 'scss', sql: 'sql',
			nix: 'nix', lua: 'lua', zig: 'zig', php: 'php',
			ex: 'elixir', exs: 'elixir', erl: 'erlang', hs: 'haskell',
		};
		return m[ext];
	}

	/** Svelte action: syntax-highlight a <code> element */
	function highlightCode(el: HTMLElement, lang: string | undefined) {
		import('highlight.js').then(({ default: hljs }) => {
			if (lang) {
				try {
					const result = hljs.highlight(el.textContent ?? '', { language: lang });
					el.innerHTML = result.value;
				} catch {
					// fallback: no highlighting
				}
			}
		});
	}

	/** Sidebar shows when there are relations OR commit refs */
	const hasSidebar = $derived(sidebarSections.size > 0 || hasCommitRefs);

	function typeFolder(type: string): string {
		return $docTypes[type]?.folder ?? type;
	}
</script>

<svelte:head>
	<title>{doc ? `${doc.id}: ${doc.title}` : (docId ?? 'Document')}</title>
</svelte:head>

<div class="mx-auto max-w-5xl">
	{#if $docsLoading}
		<div class="space-y-4 animate-pulse">
			<div class="h-5 w-32 bg-muted rounded"></div>
			<div class="rounded-xl border bg-card p-6 space-y-4">
				<div class="flex gap-2">
					<div class="h-5 w-16 bg-muted rounded"></div>
					<div class="h-5 w-20 bg-muted rounded"></div>
				</div>
				<div class="h-7 w-2/3 bg-muted rounded"></div>
				<div class="h-4 w-1/3 bg-muted rounded"></div>
				<div class="border-t pt-4 space-y-3">
					<div class="h-4 w-full bg-muted rounded"></div>
					<div class="h-4 w-5/6 bg-muted rounded"></div>
					<div class="h-4 w-4/6 bg-muted rounded"></div>
					<div class="h-6 w-1/4 bg-muted rounded mt-4"></div>
					<div class="h-4 w-full bg-muted rounded"></div>
					<div class="h-4 w-3/4 bg-muted rounded"></div>
				</div>
			</div>
		</div>
	{:else if !doc}
		<div class="text-muted-foreground">Document not found: {docId}</div>
	{:else}
		<div class="flex items-center justify-between mb-4 [&_nav]:mb-0">
			<Breadcrumb crumbs={[
				{ label: typeDisplay, href: `/${typeSlug}` },
				{ label: doc.id }
			]} />
			{#if doc.source_path}
				<SourceFileLink path={doc.source_path} />
			{/if}
		</div>

		<!-- Proposed banner -->
		{#if doc.status === 'proposed' || doc.status === 'draft'}
			<div class="mb-6 rounded-xl border border-yellow-300 bg-yellow-50 dark:bg-yellow-950/30 px-4 py-3">
				<div class="flex items-start gap-2">
					<span class="mt-0.5 text-lg leading-none">⚠</span>
					<p class="font-semibold text-foreground">This document has not been approved yet — it is still a proposal</p>
				</div>
			</div>
		{/if}

		<!-- Proposed supersession banner — shown when a proposed doc claims to supersede this one -->
		{#each doc.backlinks.filter((bl) => bl.relation === 'supersedes') as bl}
			{@const supersedingDoc = $allDocs.find((d) => d.id === bl.id)}
			{#if supersedingDoc?.status === 'proposed' || supersedingDoc?.status === 'draft'}
				{@const refType = bl.id.split('-')[0]?.toLowerCase()}
				{@const folder = typeFolder(refType)}
				<div class="mb-6 rounded-xl border border-blue-300 bg-blue-50 dark:bg-blue-950/30 px-4 py-3">
					<div class="flex items-start gap-2">
						<span class="mt-0.5 text-lg leading-none">📋</span>
						<div>
							<p class="font-semibold text-foreground">
								<a href="/{folder}/{bl.id.toLowerCase()}" class="text-primary underline hover:text-primary/80">{bl.id}</a> proposes to supersede this document
							</p>
							{#if supersedingDoc.title}
								<p class="mt-1 text-sm text-muted-foreground">{supersedingDoc.title}</p>
							{/if}
						</div>
					</div>
				</div>
			{/if}
		{/each}

		<!-- Superseded / Deprecated banner -->
		{#if doc.status === 'superseded' || doc.status === 'deprecated'}
			{@const supersededBy = doc.links?.superseded_by?.filter(Boolean) ?? []}
			<div class="mb-6 rounded-xl border {doc.status === 'superseded' ? 'border-amber-300 bg-amber-50 dark:bg-amber-950/30' : 'border-border bg-muted'} px-4 py-3">
				<div class="flex items-start gap-2">
					<span class="mt-0.5 text-lg leading-none">{doc.status === 'superseded' ? '→' : '⊘'}</span>
					<div>
						<p class="font-semibold text-foreground">
							{doc.status === 'superseded' ? 'This document has been superseded' : 'This document has been deprecated'}
						</p>
						{#if supersededBy.length > 0}
							<p class="mt-1 text-sm text-muted-foreground">
								A newer version of this decision exists. See:
								{#each supersededBy as ref, i}
									{@const refType = ref.split('-')[0]?.toLowerCase()}
									{@const folder = typeFolder(refType)}
									{#if i > 0}, {/if}
									<a href="/{folder}/{ref.toLowerCase()}" class="font-medium text-primary underline hover:text-primary/80">{ref}</a>
								{/each}
							</p>
						{/if}
					</div>
				</div>
			</div>
		{/if}

		<div class="{hasSidebar ? 'xl:grid xl:grid-cols-[1fr_260px] xl:gap-8' : ''}">
			<!-- Main article card with integrated header -->
			<article class="rounded-xl border border-t-4 {accent} bg-card text-card-foreground shadow-sm">
				<!-- Document header inside the card -->
				<div class="px-6 pt-6 pb-4">
					<!-- Badges row: ID + tags + status + severity/priority -->
					<div class="grid grid-cols-[1fr_auto] gap-2 mb-3 items-start">
						<div class="flex flex-wrap items-center gap-2">
							{#if typeDescription}
								<Tooltip.Root delayDuration={300}>
									<Tooltip.Trigger class="cursor-default">
										<Badge variant="outline" class="font-mono">{doc.id}</Badge>
									</Tooltip.Trigger>
									<Tooltip.Content sideOffset={4} class="pointer-events-none">{typeDescription}</Tooltip.Content>
								</Tooltip.Root>
							{:else}
								<Badge variant="outline" class="font-mono">{doc.id}</Badge>
							{/if}
							{#if doc.tags}
								{#each doc.tags as tag}
									<a href="/tags/{tag}" class="no-underline">
									<Badge variant="secondary" class="cursor-pointer hover:bg-secondary/80">#{tag}</Badge>
								</a>
								{/each}
							{/if}
							{#if doc.category}
								<Badge variant="secondary">{doc.category}</Badge>
							{/if}
							{#if doc.severity}
								{@const sevTip = fieldTooltip('severity', doc.severity)}
								{#if sevTip}
									<Tooltip.Root delayDuration={300}>
										<Tooltip.Trigger class="cursor-default">
											<Badge variant="outline" class={severityColor[doc.severity.toLowerCase()] ?? ''}>{doc.severity.toUpperCase()}</Badge>
										</Tooltip.Trigger>
										<Tooltip.Content sideOffset={4} class="pointer-events-none">{sevTip}</Tooltip.Content>
									</Tooltip.Root>
								{:else}
									<Badge variant="outline" class={severityColor[doc.severity.toLowerCase()] ?? ''}>{doc.severity.toUpperCase()}</Badge>
								{/if}
							{/if}
						</div>
						{#if doc.status}
							<StatusBadge status={doc.status} docType={doc.type} />
						{/if}
					</div>

					<!-- Title -->
					<h1 class="text-2xl font-bold tracking-tight text-foreground mb-3">{doc.title}</h1>

					<!-- Metadata rows -->
					{#if peopleEntries.length > 0}
						<div class="flex flex-wrap items-center gap-x-6 gap-y-1.5 text-sm text-muted-foreground">
							{#each peopleEntries as [role, handles]}
								<span class="inline-flex items-center gap-1.5">
									<span class="capitalize">{role.replace(/_/g, ' ')}:</span>
									{#each handles as handle, i}
										{@const u = resolveUser(handle)}
										{#if i > 0}<span class="text-muted-foreground/50">,</span>{/if}
										<a href="/org/users/{handle}" class="inline-flex items-center gap-1 no-underline hover:text-foreground transition-colors">
											<UserAvatar {handle} name={u?.name ?? handle} avatarUrl={u?.avatar_url} size="sm" />
											<span>{u?.name ?? `@${handle}`}</span>
										</a>
									{/each}
								</span>
							{/each}
						</div>
					{/if}
					{#if metaItems.length > 0 || badgeMeta.length > 0}
						<div class="flex flex-wrap items-center gap-x-6 gap-y-1.5 text-sm text-muted-foreground">
							{#each metaItems as item}
								<span class="inline-flex items-center gap-1">
									<span class="capitalize">{item.label}:</span>
									<span class="text-foreground">{item.value}</span>
								</span>
							{/each}
							{#if metaItems.length > 0 && badgeMeta.length > 0}
								<span class="mx-0.5 h-3.5 w-px bg-border" aria-hidden="true"></span>
							{/if}
							{#each badgeMeta as bm}
								{@const tip = fieldTooltip(bm.label.toLowerCase(), bm.value)}
								<span class="inline-flex items-center gap-1">
									<span class="capitalize">{bm.label}:</span>
									{#if tip}
										<Tooltip.Root delayDuration={300}>
											<Tooltip.Trigger class="cursor-default">
												<Badge variant="outline" class="text-xs {bm.colorClass}">{bm.value}</Badge>
											</Tooltip.Trigger>
											<Tooltip.Content sideOffset={4} class="pointer-events-none">{tip}</Tooltip.Content>
										</Tooltip.Root>
									{:else}
										<Badge variant="outline" class="text-xs {bm.colorClass}">{bm.value}</Badge>
									{/if}
								</span>
							{/each}
						</div>
					{/if}
				</div>

				<Separator />

				<!-- Body content -->
				<div class="doc-body prose prose-slate dark:prose-invert max-w-none prose-headings:font-semibold prose-a:text-primary min-w-0 p-6">
					<HtmlContent html={doc.body_html} />
				</div>
			</article>

			<!-- Sidebar: Relations + Commits -->
			{#if hasSidebar}
				<aside class="mt-8 xl:mt-0 space-y-4 xl:sticky xl:top-20 xl:self-start min-w-0">
					{#each SIDEBAR_SECTIONS as section (section.key)}
						{@const items = sidebarSections.get(section.key)}
						{#if items?.length}
							<Card.Root class="py-3 gap-2" style="border-top: 2px solid {section.accentColor};">
								<Card.Header class="pb-0">
									<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{section.label}</Card.Title>
								</Card.Header>
								<Card.Content class="grid gap-1.5">
									{#each items as item (item.id + item.label)}
										<DocRefLink refId={item.id}>
											<span class="flex flex-col gap-1 rounded-lg border bg-card p-2.5 hover:bg-accent transition-colors">
												<span class="flex items-center gap-1.5 flex-wrap">
													<span class="font-mono text-[10px] font-medium text-muted-foreground">{item.id}</span>
													<span class="text-[10px] italic text-muted-foreground/70">
														{item.label}
														<Tooltip.Root delayDuration={200}>
															<Tooltip.Trigger class="cursor-default not-italic font-medium text-muted-foreground underline decoration-dotted decoration-muted-foreground/40 underline-offset-2">{doc.id}</Tooltip.Trigger>
															<Tooltip.Content sideOffset={4} class="pointer-events-none">Current document</Tooltip.Content>
														</Tooltip.Root>
													</span>
												</span>
												<span class="text-xs font-medium leading-snug text-foreground line-clamp-2 break-words">{item.title}</span>
											</span>
										</DocRefLink>
									{/each}
								</Card.Content>
							</Card.Root>
						{/if}
					{/each}

					<!-- Git commits in sidebar -->
					{#if hasCommitRefs && docCodeRefs}
						<Card.Root class="py-3 gap-2" style="border-top: 2px solid #9ca3af;">
							<Card.Header class="pb-0">
								<Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Recent Related Commits</Card.Title>
							</Card.Header>
							<Card.Content class="grid gap-1.5">
								{#each docCodeRefs.commits.slice(0, 8) as commit}
									{@const authorHandle = resolveAuthorHandle(commit.author)}
									{@const authorUser = authorHandle ? resolveUser(authorHandle) : undefined}
									{#snippet commitInner()}
										<span class="flex items-center gap-1.5">
											<UserAvatar
												handle={authorHandle ?? commit.author}
												name={commit.author}
												avatarUrl={authorUser?.avatar_url}
												size="sm"
											/>
											<span class="text-[10px] text-muted-foreground">{commit.date}</span>
										</span>
										<span class="text-xs font-medium leading-snug text-foreground line-clamp-2 break-words" title={commit.body_context ? `${commit.subject}\n\n${commit.body_context}` : commit.subject}>{commit.subject}</span>
										{#if commit.body_context}
											<span class="text-[10px] text-muted-foreground/80 leading-snug line-clamp-2 break-words italic">{commit.body_context}</span>
										{/if}
									{/snippet}
									{#if commitUrlPrefix}
										<a href="{commitUrlPrefix}{commit.sha}" target="_blank" rel="noopener noreferrer"
											class="flex flex-col gap-1 rounded-lg border bg-card p-2.5 hover:bg-accent transition-colors no-underline">
											{@render commitInner()}
										</a>
									{:else}
										<div class="flex flex-col gap-1 rounded-lg border bg-card p-2.5">
											{@render commitInner()}
										</div>
									{/if}
								{/each}
								{#if docCodeRefs.commits.length > 8}
									<p class="text-[10px] text-muted-foreground text-center">+{docCodeRefs.commits.length - 8} more</p>
								{/if}
							</Card.Content>
						</Card.Root>
					{/if}
				</aside>
			{/if}
		</div>

		<!-- Code file references (full width, below the grid) -->
		{#if hasCodeFileRefs}
			<div class="mt-6 space-y-3">
				<h3 class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Source Code References</h3>
				{#each codeRefsByFile as fileGroup}
					<div class="rounded-lg border bg-card overflow-hidden">
						<div class="bg-muted/50 px-4 py-2 border-b flex items-center gap-2">
							{#if fileGroup.baseFileUrl}
								<a href={fileGroup.baseFileUrl} target="_blank" rel="noopener noreferrer"
									class="text-xs font-mono text-primary hover:underline">{fileGroup.file}</a>
							{:else}
								<span class="text-xs font-mono text-primary">{fileGroup.file}</span>
							{/if}
						</div>
						<div class="overflow-x-auto">
							{#each fileGroup.blocks as block}
								{#if 'gap' in block}
									<div class="flex items-center border-b border-border/30 px-3 py-0.5 bg-muted/20 select-none">
										<span class="w-14 shrink-0"></span>
										<span class="text-xs font-mono text-muted-foreground">···</span>
									</div>
								{:else}
									<div class="flex items-baseline border-b border-border/30 last:border-0 {block.isMatch ? 'bg-yellow-50 dark:bg-yellow-950/20' : 'hover:bg-muted/30'}">
										{#if fileGroup.baseFileUrl && block.isMatch}
											<a href="{fileGroup.baseFileUrl}#L{block.lineNum}" target="_blank" rel="noopener noreferrer"
												class="shrink-0 w-14 text-right pr-3 py-1.5 text-xs font-mono text-muted-foreground hover:text-primary select-none no-underline">{block.lineNum}</a>
										{:else}
											<span class="shrink-0 w-14 text-right pr-3 py-1.5 text-xs font-mono text-muted-foreground select-none">{block.lineNum}</span>
										{/if}
										<code class="py-1.5 pr-4 text-xs font-mono whitespace-pre {block.isMatch ? '' : 'opacity-60'}" use:highlightCode={extToLang(fileGroup.file)}>{block.text}</code>
									</div>
								{/if}
							{/each}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</div>
