<script lang="ts">
	import { docTypes, allDocs, docsLoading } from '$lib/stores/docs';
	import { orgData } from '$lib/stores/org';
	import { assignmentsData, loadAssignments } from '$lib/stores/assignments';
	import { isDark } from '$lib/stores/theme';
	import { schemaData } from '$lib/stores/schema';
	import { graphNodes, graphEdges, loadGraph } from '$lib/stores/graph';
	import { getEdgeStyle, EXCLUDED_RELATIONS, CATEGORIES } from '$lib/config/relations';
	import dagre from '@dagrejs/dagre';
	import {
		SvelteFlow,
		Background,
		BackgroundVariant,
		Position,
		type Node,
		type Edge
	} from '@xyflow/svelte';
	import '@xyflow/svelte/dist/style.css';
	import DocNode from '$lib/components/graph/DocNode.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import { enrichContentRefs } from '$lib/actions/content-refs';
	import { goto } from '$app/navigation';
	import { setContext, untrack, onMount } from 'svelte';
	import { writable } from 'svelte/store';
	import LifecycleFlow from '$lib/components/LifecycleFlow.svelte';
	import BoxesIcon from '@lucide/svelte/icons/boxes';
	import LightbulbIcon from '@lucide/svelte/icons/lightbulb';
	import ShieldIcon from '@lucide/svelte/icons/shield';
	import AlertTriangleIcon from '@lucide/svelte/icons/triangle-alert';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import type { Component } from 'svelte';

	onMount(() => {
		loadAssignments();
	});

	const docTypeIcons: Record<string, Component> = {
		opp: LightbulbIcon,
		pol: ShieldIcon,
		adr: BoxesIcon,
		spec: FileTextIcon,
		inc: AlertTriangleIcon
	};

	const typeList = $derived(Object.entries($docTypes));

	const docTypeCards: {
		key: string;
		title: string;
		description: string;
		descriptionLink?: { text: string; href: string };
		when: string;
		examples: string[];
	}[] = [
		{
			key: 'opp',
			title: 'Opportunities',
			description:
				'The starting point. Opportunities capture business or technical initiatives worth pursuing. They combine discovery ("why") with requirements ("what"), so decisions stay connected to the value they deliver.',
			when: 'A new feature idea, market opportunity, cost-saving initiative, or technical improvement is identified.',
			examples: [
				'Migrate payment processing to reduce transaction fees',
				'Launch mobile app to capture underserved user segment',
				'Consolidate three microservices to cut operational overhead'
			]
		},
		{
			key: 'pol',
			title: 'Policies',
			description:
				'Some opportunities involve regulated areas (data retention, licensing, compliance) or the organization wants to be explicit about working styles. Policies capture these rules and constraints that shape every other decision.',
			when: 'A rule, constraint, or standard needs documenting so that both humans and AI agents understand the boundaries.',
			examples: [
				'All PII must be encrypted at rest (regulatory)',
				'Azure commitment: 3-year reserved instances (vendor)',
				'Maximum 2 programming languages per service (engineering standard)'
			]
		},
		{
			key: 'adr',
			title: 'Architecture Decisions',
			description:
				'When technology is needed to implement opportunities, ADRs track the significant choices — what was decided, why, and what the consequences are. Compatible with',
			descriptionLink: { text: 'MADR 4.0', href: 'https://adr.github.io/madr/' },
			when: 'A technology choice, design pattern, or infrastructure decision is made that will be hard to reverse or that future developers need to understand.',
			examples: [
				'Use PostgreSQL over DynamoDB for transactional data',
				'Adopt event sourcing for the billing domain',
				'Standardize on gRPC for internal service communication'
			]
		},
		{
			key: 'spec',
			title: 'Specifications',
			description:
				'Complex opportunities need verification. Specs define exact behavior using user stories and Gherkin scenarios so you know when something is "done".',
			when: 'An opportunity moves to "pursuing" and needs concrete, testable requirements before engineering work begins.',
			examples: [
				'User can reset password via email verification',
				'Bulk import handles CSV files up to 100MB',
				'Rate limiter returns 429 after 100 requests/minute'
			]
		},
		{
			key: 'inc',
			title: 'Incidents',
			description:
				'When problems arise in production, incident reports capture what happened, root cause analysis, and action items. Like post-mortems. Lessons feed back into new policies and architecture decisions.',
			when: 'A customer-facing outage, data breach, security incident, or significant process failure occurs and needs a post-mortem.',
			examples: [
				'Database connection pool exhaustion caused 45min outage',
				'Leaked API keys in public repository',
				'Deployment pipeline failure blocked releases for 2 days'
			]
		}
	];

	/** Build the org actor lines if legal entities are defined */
	const orgNames = $derived(
		$orgData?.orgs ? Object.values($orgData.orgs).filter((o) => !o.parent).map((o) => o.name) : []
	);

	/** All orgs (including children) */
	const allOrgs = $derived(
		$orgData?.orgs ? Object.entries($orgData.orgs) : []
	);

	/** Total people count */
	const userCount = $derived(
		$orgData?.users ? Object.keys($orgData.users).length : 0
	);
	const activeTeams = $derived(
		$orgData?.teams ? Object.entries($orgData.teams).filter(([, t]) => t.status !== 'deprecated') : []
	);

	/** Open tasks: action items + requirements that are in-progress or pending */
	const openTasks = $derived.by(() => {
		if (!$assignmentsData) return [];
		const tasks: { description: string; status: string; due_date?: string; doc_id: string; doc_type: string; doc_title: string; owner: string; section: string }[] = [];
		for (const [handle, assignments] of Object.entries($assignmentsData.users)) {
			for (const a of assignments) {
				if (a.role !== 'table_action_items' && a.role !== 'table_requirements') continue;
				const s = a.status?.toLowerCase();
				if (!s || s === 'completed') continue;
				tasks.push({
					description: a.description ?? '',
					status: a.status ?? '',
					due_date: a.due_date,
					doc_id: a.doc_id,
					doc_type: a.doc_type,
					doc_title: a.doc_title,
					owner: handle,
					section: a.section ?? ''
				});
			}
		}
		// Sort: overdue first, then by due date
		return tasks.sort((a, b) => {
			if (a.due_date && b.due_date) return a.due_date.localeCompare(b.due_date);
			if (a.due_date) return -1;
			if (b.due_date) return 1;
			return a.doc_id.localeCompare(b.doc_id);
		});
	});

	const mermaidDef = $derived.by(() => {
		const lines = ['flowchart TD'];

		// Organization (top)
		if (orgNames.length > 0) {
			const label = orgNames.length === 1 ? orgNames[0] : orgNames.join(', ');
			lines.push(
				'    subgraph org ["Organization"]',
				`        ORG(("${label}"))`,
				'    end'
			);
		}

		// Documents (middle)
		lines.push(
			'    subgraph docs ["Documents"]',
			'        OPP["<b>Opportunities</b>"]',
			'        POL["Policies"]',
			'        ADR["Architecture Decisions"]',
			'        SPEC["Specifications"]',
			'        INC["Incidents"]',
			'    end'
		);

		// Users & Software (bottom)
		lines.push(
			'    USERS(["Users"])',
			'    subgraph sw ["Software"]',
			'        SW["What we ship"]',
			'    end',
			''
		);

		// Org → docs edges
		if (orgNames.length > 0) {
			lines.push(
				'    ORG -->|"strategic direction"| OPP',
				'    ORG -->|"compliance"| POL',
				'    POL -.->|"defines how we work"| ORG'
			);
		}

		// Document interconnections
		lines.push(
			orgNames.length > 0
				? '    OPP -->|"regulated area"| POL'
				: '    OPP -->|"rules & constraints"| POL',
			'    OPP -->|"technical implementation"| ADR',
			'    OPP -->|"testable requirements"| SPEC',
			'    ADR -.->|"if problems arise"| INC',
			'    SPEC -.->|"if problems arise"| INC',
			'    INC -.->|"lessons learned"| POL',
			'    INC -.->|"lessons learned"| ADR'
		);

		// Docs → Software (downward)
		lines.push(
			'    ADR -->|"guides build"| SW',
			'    SPEC -->|"defines behavior"| SW',
			'    POL -->|"constrains"| SW'
		);

		// Software feedback
		lines.push(
			'    SW -.->|"can cause"| INC',
			'    OPP -.->|"fulfilled by"| SW'
		);

		// Value chain: Users → Software, Users → Organization
		lines.push('    SW -->|"delivers value"| USERS');
		if (orgNames.length > 0) {
			lines.push('    USERS -.->|"generates revenue"| ORG');
		}

		// Styling
		if (orgNames.length > 0) {
			lines.push('    style org fill:#8b5cf620,stroke:#8b5cf6,stroke-width:1px,color:#8b5cf6');
		}
		lines.push(
			'    style docs fill:#3b82f620,stroke:#3b82f6,stroke-width:1px,color:#3b82f6',
			'    style sw fill:#10b98120,stroke:#10b981,stroke-width:1px,color:#10b981'
		);

		return lines.join('\n');
	});

	let containerEl: HTMLDivElement | undefined = $state();

	async function renderDiagram(el: HTMLDivElement, dark: boolean, _def: string) {
		try {
			const mermaid = (await import('mermaid')).default;
			mermaid.initialize({
				startOnLoad: false,
				theme: dark ? 'dark' : 'default',
				themeVariables: dark ? { darkMode: true } : {},
				flowchart: { useMaxWidth: true, curve: 'basis' }
			});
			const id = `onboarding-flow-${Math.random().toString(36).slice(2, 9)}`;
			const { svg } = await mermaid.render(id, _def);
			el.innerHTML = svg;

			const svgEl = el.querySelector('svg');
			if (svgEl) {
				svgEl.style.width = '100%';
				svgEl.style.height = 'auto';
				svgEl.style.maxHeight = '600px';

				// Move subgraph labels to the left edge of their cluster box
				svgEl.querySelectorAll('g.cluster').forEach((cluster) => {
					const rect = cluster.querySelector(':scope > rect');
					const labelG = cluster.querySelector(':scope > g.cluster-label');
					if (!rect || !labelG) return;
					const rectX = parseFloat(rect.getAttribute('x') ?? '0');
					const fo = labelG.querySelector('foreignObject');
					const div = fo?.querySelector('div, p, span') as HTMLElement | null;
					if (fo && div) {
						// Reset the label group transform and position foreignObject at rect left + padding
						labelG.setAttribute('transform', '');
						fo.setAttribute('x', String(rectX + 8));
						fo.setAttribute('y', rect.getAttribute('y') ?? '0');
						div.style.textAlign = 'left';
					}
				});
			}
		} catch (err) {
			console.error('Onboarding mermaid render failed:', err);
			el.innerHTML = '';
		}
	}

	$effect(() => {
		const dark = $isDark;
		const def = mermaidDef;
		const el = containerEl;
		if (!el) return;
		renderDiagram(el, dark, def);
	});

	/** Hardcoded fallback statuses for types not in the project schema */
	const fallbackStatuses: Record<string, { name: string; transitions: string[] }[]> = {
		opp: [
			{ name: 'identified', transitions: ['validating', 'declined'] },
			{ name: 'validating', transitions: ['pursuing', 'declined'] },
			{ name: 'pursuing', transitions: ['completed', 'declined'] },
			{ name: 'completed', transitions: ['deprecated'] },
			{ name: 'deprecated', transitions: [] },
			{ name: 'declined', transitions: [] }
		],
		pol: [
			{ name: 'proposed', transitions: ['active'] },
			{ name: 'active', transitions: ['deprecated', 'superseded'] },
			{ name: 'deprecated', transitions: [] },
			{ name: 'superseded', transitions: [] }
		],
		adr: [
			{ name: 'proposed', transitions: ['accepted', 'rejected'] },
			{ name: 'accepted', transitions: ['deprecated', 'superseded'] },
			{ name: 'rejected', transitions: [] },
			{ name: 'deprecated', transitions: [] },
			{ name: 'superseded', transitions: [] }
		],
		spec: [
			{ name: 'proposed', transitions: ['approved'] },
			{ name: 'approved', transitions: ['implemented', 'deprecated'] },
			{ name: 'implemented', transitions: ['deprecated'] },
			{ name: 'deprecated', transitions: [] }
		],
		inc: [
			{ name: 'open', transitions: ['mitigated', 'resolved'] },
			{ name: 'mitigated', transitions: ['resolved'] },
			{ name: 'resolved', transitions: [] }
		]
	};

	/** Build lifecycle nodes with transitions for LifecycleFlow component */
	function lifecycleNodes(key: string) {
		const schema = $schemaData;
		const statuses = schema?.types[key]?.statuses ?? [];
		if (statuses.length === 0) return fallbackStatuses[key] ?? [];
		// If transitions already defined in schema, use as-is
		if (statuses.some((s) => s.transitions?.length)) return statuses;
		// Otherwise, generate a linear chain: each status transitions to the next
		return statuses.map((s, i) => ({
			...s,
			transitions: i < statuses.length - 1 ? [statuses[i + 1].name] : []
		}));
	}

	// --- Mini-graph: show the most-connected document ---
	const miniNodeTypes = { doc: DocNode };
	const MINI_NODE_W = 360;
	const MINI_NODE_H = 56;

	// Share hover state with DocNode via context
	const miniHoveredStore = writable<Set<string>>(new Set());
	setContext('graphHighlight', miniHoveredStore);

	// Load graph data
	$effect(() => {
		loadGraph();
	});

	/** Derive doc type prefix from ID (e.g. ADR-001 → adr) */
	function docTypeFromId(id: string): string {
		return id.split('-')[0].toLowerCase();
	}

	/** Find the most-connected node and build its neighborhood */
	const miniGraphData = $derived.by(() => {
		const nodes = $graphNodes;
		const edges = $graphEdges;
		if (nodes.length === 0 || edges.length === 0) return null;

		const filteredEdges = edges.filter((e) => !EXCLUDED_RELATIONS.has(e.relation));

		// Count connections per node
		const counts = new Map<string, number>();
		for (const e of filteredEdges) {
			counts.set(e.source, (counts.get(e.source) ?? 0) + 1);
			counts.set(e.target, (counts.get(e.target) ?? 0) + 1);
		}

		// Find most connected
		let bestId = '';
		let bestCount = 0;
		for (const [id, count] of counts) {
			if (count > bestCount) {
				bestId = id;
				bestCount = count;
			}
		}
		if (!bestId) return null;

		// Collect neighborhood
		const neighborIds = new Set<string>([bestId]);
		const neighborEdges = filteredEdges.filter((e) => {
			if (e.source === bestId || e.target === bestId) {
				neighborIds.add(e.source);
				neighborIds.add(e.target);
				return true;
			}
			return false;
		});

		const neighborNodes = nodes.filter((n) => neighborIds.has(n.id));
		const centerNode = nodes.find((n) => n.id === bestId);

		return { centerNode, neighborNodes, neighborEdges, bestId, bestCount };
	});

	let miniFlowNodes = $state<Node[]>([]);
	let miniFlowEdges = $state<Edge[]>([]);
	let miniHoveredNodeId = $state<string | null>(null);

	$effect(() => {
		const data = miniGraphData;
		if (!data) {
			miniFlowNodes = [];
			miniFlowEdges = [];
			return;
		}

		const g = new dagre.graphlib.Graph();
		g.setGraph({ rankdir: 'TB', ranksep: 80, nodesep: 30, edgesep: 20 });
		g.setDefaultEdgeLabel(() => ({}));

		for (const n of data.neighborNodes) {
			g.setNode(n.id, { width: MINI_NODE_W, height: MINI_NODE_H });
		}
		for (const e of data.neighborEdges) {
			g.setEdge(e.source, e.target);
		}

		dagre.layout(g);

		miniFlowNodes = data.neighborNodes.map((n) => {
			const pos = g.node(n.id);
			return {
				id: n.id,
				type: 'doc',
				position: { x: pos.x - MINI_NODE_W / 2, y: pos.y - MINI_NODE_H / 2 },
				data: {
					label: n.id,
					title: n.title,
					docType: docTypeFromId(n.id),
					status: n.status
				},
				sourcePosition: Position.Bottom,
				targetPosition: Position.Top
			};
		});

		miniFlowEdges = data.neighborEdges.map((e) => {
			const es = getEdgeStyle(e.relation);
			const dashPart = es.strokeDasharray ? ` stroke-dasharray: ${es.strokeDasharray};` : '';
			const opacityPart = es.opacity < 1 ? ` opacity: ${es.opacity};` : '';
			return {
				id: `mini-${e.source}-${e.relation}-${e.target}`,
				source: e.source,
				target: e.target,
				label: e.relation,
				type: 'smoothstep',
				animated: false,
				data: { relation: e.relation },
				style: `stroke: ${es.color}; stroke-width: ${es.strokeWidth};${dashPart}${opacityPart}`,
				labelStyle: `fill: ${es.color}; font-size: 10px; font-weight: 500;${opacityPart}`,
				labelBgStyle: 'fill: var(--card); fill-opacity: 0.85;',
				markerEnd: es.markerEnd ? { type: 'arrowclosed' as const, color: es.color } : undefined
			};
		});
	});

	// Hover highlight for mini-graph
	$effect(() => {
		const hId = miniHoveredNodeId;
		const edges = untrack(() => miniFlowEdges);

		if (!hId) {
			miniHoveredStore.set(new Set());
			miniFlowEdges = edges.map((e) => {
				const relation = (e.data?.relation as string) ?? '';
				const es = getEdgeStyle(relation);
				const dashPart = es.strokeDasharray ? ` stroke-dasharray: ${es.strokeDasharray};` : '';
				const opacityPart = es.opacity < 1 ? ` opacity: ${es.opacity};` : '';
				return {
					...e,
					style: `stroke: ${es.color}; stroke-width: ${es.strokeWidth};${dashPart}${opacityPart}`,
					labelStyle: `fill: ${es.color}; font-size: 10px; font-weight: 500;${opacityPart}`,
					labelBgStyle: 'fill: var(--card); fill-opacity: 0.85;'
				};
			});
			return;
		}

		const hlNodes = new Set<string>([hId]);
		const hlEdgeIds = new Set<string>();
		for (const e of edges) {
			if (e.source === hId || e.target === hId) {
				hlNodes.add(e.source);
				hlNodes.add(e.target);
				hlEdgeIds.add(e.id);
			}
		}

		miniHoveredStore.set(hlNodes);

		miniFlowEdges = edges.map((e) => {
			const relation = (e.data?.relation as string) ?? '';
			const es = getEdgeStyle(relation);
			const dimmed = !hlEdgeIds.has(e.id);
			const dashPart = es.strokeDasharray ? ` stroke-dasharray: ${es.strokeDasharray};` : '';
			const opacityPart = dimmed ? ' opacity: 0.1;' : (es.opacity < 1 ? ` opacity: ${es.opacity};` : '');
			return {
				...e,
				style: `stroke: ${es.color}; stroke-width: ${es.strokeWidth};${dashPart}${opacityPart}`,
				labelStyle: `fill: ${es.color}; font-size: 10px; font-weight: 500;${dimmed ? ' opacity: 0.1;' : ''}`,
				labelBgStyle: `fill: var(--card); fill-opacity: 0.85;${dimmed ? ' opacity: 0.1;' : ''}`
			};
		});
	});

	function onMiniNodePointerEnter({ node }: { node: Node; event: PointerEvent }) {
		miniHoveredNodeId = node.id;
	}
	function onMiniNodePointerLeave() {
		miniHoveredNodeId = null;
	}
	function onMiniNodeClick({ node }: { node: Node; event: MouseEvent | TouchEvent }) {
		const type = docTypeFromId(node.id);
		const folder = $docTypes[type]?.folder ?? type;
		goto(`/${folder}/${node.id.toLowerCase()}`);
	}
</script>

<svelte:head>
	<title>Getting Started</title>
</svelte:head>

<div class="mx-auto max-w-4xl">
	<h1 class="text-2xl font-bold text-foreground mb-2">Getting Started</h1>
	<p class="text-muted-foreground mb-8 leading-relaxed">
		DecisionGraph helps your organization capture what it's building and why. Structured markdown
		files — versioned, linked, and validated — replace decisions buried in Slack threads or
		forgotten Google Docs. Everything starts from <strong>Opportunities</strong>.
	</p>

	{#if $docsLoading}
		<div class="text-muted-foreground">Loading...</div>
	{:else}
		<!-- Decision flow graph -->
		<section class="mb-10">
			<h2 class="text-lg font-semibold text-foreground mb-2">How documents connect</h2>
			<p class="text-sm text-muted-foreground mb-4">
				Opportunities are the center of the graph. Other document types branch out from them
				as needed — policies when rules apply, ADRs when technical choices arise, specs when
				behavior needs verification, and incidents when things go wrong. Lessons from
				incidents feed back into policies and decisions. Here's how it all connects:
			</p>
			<div
				bind:this={containerEl}
				class="overflow-x-auto rounded-lg border border-border bg-card p-4"
			></div>
		</section>

		<!-- Document types detail -->
		<section class="mb-10">
			<h2 class="text-lg font-semibold text-foreground mb-4">Document types</h2>
			<div class="space-y-6">
				{#each docTypeCards as card}
					{@const info = $docTypes[card.key]}
					{@const CardIcon = docTypeIcons[card.key]}
					<div class="rounded-lg border border-border bg-card p-5 shadow-sm">
						<div class="flex items-center justify-between mb-2">
							<a
								href="/{info?.folder ?? card.key}"
								class="text-base font-semibold text-primary hover:underline flex items-center gap-2"
							>
								{#if CardIcon}<CardIcon class="size-4" />{/if}
								{card.title}
							</a>
							<span class="text-xs text-muted-foreground font-mono"
								>./docs/{info?.folder ?? card.key}/{card.key.toUpperCase()}-*.md</span
							>
						</div>
						<p class="text-sm text-foreground leading-relaxed mb-3">
							{card.description}{#if card.descriptionLink}
								{' '}<a href={card.descriptionLink.href} target="_blank" rel="noopener noreferrer" class="text-primary hover:underline">{card.descriptionLink.text}</a>.{/if}
						</p>
						<div class="text-xs text-muted-foreground mb-2">
							<span class="font-medium text-foreground">When to create one:</span>
							{card.when}
						</div>
						<div class="text-xs text-muted-foreground">
							<span class="font-medium text-foreground">Examples:</span>
							<ul class="mt-1 ml-4 list-disc space-y-0.5">
								{#each card.examples as ex}
									<li>{ex}</li>
								{/each}
							</ul>
						</div>
							{#if $allDocs.filter((d) => d.type === card.key).length > 0}
							<div class="mt-3 text-xs">
								<a href="/{info?.folder ?? card.key}" class="text-primary hover:underline">
									Your project has {$allDocs.filter((d) => d.type === card.key).length}
									{card.title.toLowerCase()}
									{$allDocs.filter((d) => d.type === card.key).length === 1 ? 'document' : 'documents'}.
								</a>
							</div>
						{:else}
							<div class="mt-3 text-xs text-muted-foreground">
								You don't yet have any {card.title.toLowerCase()}.
							</div>
						{/if}
						{#if lifecycleNodes(card.key).length > 0}
							<div class="mt-3">
								<div class="text-xs font-medium text-foreground mb-1">Each document follows a <strong>status lifecycle</strong>:</div>
								<div class="overflow-x-auto rounded border border-border/50 bg-muted/30 px-3 py-2">
									<LifecycleFlow nodes={lifecycleNodes(card.key)} docType={card.key} />
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		</section>

		<!-- Relations -->
		<section class="mb-10">
			<h2 class="text-lg font-semibold text-foreground mb-2">Relations</h2>
			<p class="text-sm text-muted-foreground mb-4">
				Documents link to each other through typed relations in their YAML frontmatter. These
				relations are validated automatically and create a navigable graph.
			</p>
			<div class="overflow-x-auto">
				<table class="w-full border-collapse text-sm">
					<thead>
						<tr class="border-b border-border">
							<th class="py-2 pr-4 text-left font-medium text-muted-foreground">Relation</th>
							<th class="py-2 pr-4 text-left font-medium text-muted-foreground">Meaning</th>
							<th class="py-2 text-left font-medium text-muted-foreground">Example</th>
						</tr>
					</thead>
					<tbody>
						<tr class="border-b border-border/50">
							<td class="py-2 pr-4 font-mono text-xs">implements</td>
							<td class="py-2 pr-4">Technical realization of an opportunity or policy</td>
							<td class="py-2 text-xs text-muted-foreground italic"
								>SPEC-001 implements OPP-003</td
							>
						</tr>
						<tr class="border-b border-border/50">
							<td class="py-2 pr-4 font-mono text-xs">depends_on</td>
							<td class="py-2 pr-4">Cannot proceed until the target is resolved</td>
							<td class="py-2 text-xs text-muted-foreground italic"
								>ADR-002 depends_on ADR-001</td
							>
						</tr>
						<tr class="border-b border-border/50">
							<td class="py-2 pr-4 font-mono text-xs">enables</td>
							<td class="py-2 pr-4"
								>Prerequisite &mdash; target can exist but can't succeed without source</td
							>
							<td class="py-2 text-xs text-muted-foreground italic"
								>POL-001 enables OPP-002</td
							>
						</tr>
						<tr class="border-b border-border/50">
							<td class="py-2 pr-4 font-mono text-xs">triggers</td>
							<td class="py-2 pr-4"
								>Direct cause &mdash; target was created because of source</td
							>
							<td class="py-2 text-xs text-muted-foreground italic"
								>INC-001 triggers POL-003</td
							>
						</tr>
						<tr class="border-b border-border/50">
							<td class="py-2 pr-4 font-mono text-xs">supersedes</td>
							<td class="py-2 pr-4">Replaces a previous document entirely</td>
							<td class="py-2 text-xs text-muted-foreground italic"
								>ADR-005 supersedes ADR-001</td
							>
						</tr>
						<tr class="border-b border-border/50">
							<td class="py-2 pr-4 font-mono text-xs">conflicts_with</td>
							<td class="py-2 pr-4">Contradicts or creates tension with another document</td>
							<td class="py-2 text-xs text-muted-foreground italic"
								>OPP-004 conflicts_with POL-001</td
							>
						</tr>
					</tbody>
				</table>
			</div>
		</section>

		<!-- Example graph: most-connected document -->
		{#if miniGraphData}
			<section class="mb-10">
				<h2 class="text-lg font-semibold text-foreground mb-2">Example: how a document connects</h2>
				<p class="text-sm text-muted-foreground mb-4">
					The most connected document in your project is
					<a href="/{$docTypes[docTypeFromId(miniGraphData.bestId)]?.folder ?? docTypeFromId(miniGraphData.bestId)}/{miniGraphData.bestId.toLowerCase()}" class="text-primary font-medium hover:underline">{miniGraphData.bestId}</a>
					({miniGraphData.centerNode?.title}) with {miniGraphData.bestCount} connections.
					{#if miniGraphData.neighborEdges.some((e) => e.relation === 'enables')}
						It enables opportunities,
					{/if}
					{#if miniGraphData.neighborEdges.some((e) => e.relation === 'triggers')}
						triggers new policies,
					{/if}
					{#if miniGraphData.neighborEdges.some((e) => e.relation === 'related')}
						and relates to other architecture decisions.
					{:else}
						creating a web of interconnected decisions.
					{/if}
					Hover over any node to highlight its connections. Click a node to navigate to it.
				</p>

				<div class="relative rounded-xl border bg-card shadow-sm" style="height: 400px;">
					<SvelteFlow
						bind:nodes={miniFlowNodes}
						bind:edges={miniFlowEdges}
						nodeTypes={miniNodeTypes}
						fitView
						nodesDraggable={false}
						nodesConnectable={false}
						elementsSelectable={false}
						onnodeclick={onMiniNodeClick}
						onnodepointerenter={onMiniNodePointerEnter}
						onnodepointerleave={onMiniNodePointerLeave}
					>
						<Background variant={BackgroundVariant.Dots} gap={16} size={1} />
					</SvelteFlow>
				</div>

				<div class="mt-3 flex gap-3 text-xs text-muted-foreground justify-end">
					{#each Object.entries(CATEGORIES) as [, cat] (cat.label)}
						<span class="flex items-center gap-1">
							<svg width="20" height="8" class="shrink-0">
								<line x1="0" y1="4" x2="20" y2="4"
									stroke={cat.color}
									stroke-width={cat.strokeWidth}
									stroke-dasharray={cat.dasharray || 'none'}
									opacity={cat.opacity} />
								{#if cat.arrow}
									<polygon points="15,1 20,4 15,7" fill={cat.color} opacity={cat.opacity} />
								{/if}
							</svg>
							{cat.label}
						</span>
					{/each}
				</div>
			</section>
		{/if}

		<!-- People & Organization -->
		{#if $orgData && (Object.keys($orgData.teams).length > 0 || Object.keys($orgData.users).length > 0)}
			<section class="mb-10">
				<h2 class="text-lg font-semibold text-foreground mb-2">People & Organization</h2>
				<p class="text-sm text-muted-foreground mb-4">
					People, teams, and legal entities are defined in <code class="text-xs bg-muted px-1 py-0.5 rounded">.dg/org.kdl</code>.
					DecisionGraph uses this to track <strong>ownership</strong> of documents, services, and tasks.
					Frontmatter fields like
					<code class="text-xs bg-muted px-1 py-0.5 rounded">author</code>,
					<code class="text-xs bg-muted px-1 py-0.5 rounded">owner</code>, and
					<code class="text-xs bg-muted px-1 py-0.5 rounded">commander</code>
					link documents to people. Table columns like "Owner" in Action Items and Requirements
					track who is responsible for each task.
				</p>

				<!-- Teams -->
				{#if activeTeams.length > 0}
					<h3 class="text-sm font-semibold text-foreground mb-2">
						Teams
						<span class="font-normal text-muted-foreground">({activeTeams.length} teams, {userCount} people)</span>
					</h3>
					<div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 mb-6">
						{#each activeTeams as [id, team]}
							<a
								href="/org/teams/{id}"
								class="rounded-lg border border-border bg-card p-4 shadow-sm hover:shadow-md transition-shadow"
							>
								<div class="font-semibold text-foreground">{team.name}</div>
								<div class="text-xs text-muted-foreground">
									{team.members.length}
									{team.members.length === 1 ? 'member' : 'members'}
								</div>
								{#if team.lead}
									<div class="mt-1 text-xs text-muted-foreground">Lead: @{team.lead}</div>
								{/if}
								{#if team.org}
									<div class="mt-1 text-xs text-muted-foreground italic">{$orgData.orgs[team.org]?.name ?? team.org}</div>
								{/if}
								{#if team.description}
									<p class="mt-2 text-sm text-muted-foreground line-clamp-2">
										{team.description}
									</p>
								{/if}
							</a>
						{/each}
					</div>
				{/if}

				<!-- Open tasks -->
				{#if openTasks.length > 0}
					<h3 class="text-sm font-semibold text-foreground mb-2">
						Open tasks
						<span class="font-normal text-muted-foreground">({openTasks.length} across all documents)</span>
					</h3>
					<p class="text-sm text-muted-foreground mb-3">
						Action items from incident post-mortems and requirements from active opportunities
						are tracked per person. Here are some that are currently in progress or pending:
					</p>
					<div class="mb-6">
						<table class="w-full border-collapse text-sm">
							<thead>
								<tr class="border-b border-border">
									<th class="py-2 pr-4 text-left font-medium text-muted-foreground">Task</th>
									<th class="py-2 pr-4 text-left font-medium text-muted-foreground">Document</th>
									<th class="py-2 pr-4 text-left font-medium text-muted-foreground">Owner</th>
									<th class="py-2 pr-4 text-left font-medium text-muted-foreground">Status</th>
									<th class="py-2 text-left font-medium text-muted-foreground">Due</th>
								</tr>
							</thead>
							<tbody>
								{#each openTasks.slice(0, 5) as task}
									{@const folder = $docTypes[task.doc_type]?.folder ?? task.doc_type}
									<tr class="border-b border-border/50">
										<td class="py-2 pr-4 text-foreground" use:enrichContentRefs>{task.description}</td>
										<td class="py-2 pr-4">
											<a href="/{folder}/{task.doc_id.toLowerCase()}" class="text-primary hover:underline font-mono text-xs">{task.doc_id}</a>
										</td>
										<td class="py-2 pr-4">
											<a href="/org/users/{task.owner}" class="text-primary hover:underline text-xs">@{task.owner}</a>
										</td>
										<td class="py-2 pr-4">
											<StatusBadge status={task.status} />
										</td>
										<td class="py-2 text-xs text-muted-foreground whitespace-nowrap">{task.due_date ?? ''}</td>
									</tr>
								{/each}
							</tbody>
						</table>
						{#if openTasks.length > 5}
							<p class="text-xs text-muted-foreground mt-2">
								...and {openTasks.length - 5} more. See all assignments on the <a href="/kanban" class="text-primary hover:underline">kanban board</a>.
							</p>
						{/if}
					</div>
				{/if}

				<!-- Legal entities -->
				{#if allOrgs.length > 0}
					<h3 class="text-sm font-semibold text-foreground mb-2">Legal entities</h3>
					<p class="text-sm text-muted-foreground mb-3">
						Larger organizations often operate through multiple legal entities — regional subsidiaries,
						holding companies, or acquired brands. DecisionGraph tracks these in
						<code class="text-xs bg-muted px-1 py-0.5 rounded">org.kdl</code> so
						teams and people can be associated with the entity they belong to. Entities
						appear in team and user pages and help you understand which part of the
						organization owns which decisions and services.
					</p>
					<div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 mb-2">
						{#each allOrgs as [id, org]}
							<a
								href="/org/entities/{id}"
								class="rounded-lg border border-border bg-card p-3 shadow-sm hover:shadow-md transition-shadow"
							>
								<div class="font-semibold text-foreground text-sm">{org.name}</div>
								{#if org.parent}
									<div class="text-xs text-muted-foreground">
										Subsidiary of {$orgData.orgs[org.parent]?.name ?? org.parent}
									</div>
								{:else}
									<div class="text-xs text-muted-foreground">Parent entity</div>
								{/if}
							</a>
						{/each}
					</div>
				{:else}
					<div class="text-sm text-muted-foreground mb-2">
						<strong>Legal entities:</strong> If your organization has multiple legal entities
						(regional subsidiaries, holding companies), you can define them in
						<code class="text-xs bg-muted px-1 py-0.5 rounded">org.kdl</code> with
						<code class="text-xs bg-muted px-1 py-0.5 rounded">org</code> blocks and
						<code class="text-xs bg-muted px-1 py-0.5 rounded">parent</code> fields.
						Teams and users can then be assigned to specific entities.
					</div>
				{/if}
			</section>
		{/if}

		<!-- Quick start -->
		<section class="mb-10">
			<h2 class="text-lg font-semibold text-foreground mb-2">Quick start with the CLI</h2>
			<div class="rounded-lg border border-border bg-muted/50 p-4 text-sm space-y-3 font-mono">
				<div>
					<span class="text-muted-foreground"># Create a new opportunity</span><br />
					<span class="text-foreground">dg new opp "Migrate to edge CDN"</span>
				</div>
				<div>
					<span class="text-muted-foreground"># Create a spec that implements it</span><br />
					<span class="text-foreground"
						>dg new spec "CDN cache invalidation" --set implements=OPP-004</span
					>
				</div>
				<div>
					<span class="text-muted-foreground"># Validate all documents</span><br />
					<span class="text-foreground">dg validate</span>
				</div>
				<div>
					<span class="text-muted-foreground"># See what needs attention</span><br />
					<span class="text-foreground">dg suggest</span>
				</div>
				<div>
					<span class="text-muted-foreground"># Render this site locally and open it</span><br />
					<span class="text-foreground">dg serve --open</span>
				</div>
			</div>
		</section>
	{/if}
</div>
