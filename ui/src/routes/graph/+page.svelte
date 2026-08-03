<script lang="ts">
	import dagre from '@dagrejs/dagre';
	import {
		SvelteFlow,
		Background,
		Controls,
		BackgroundVariant,
		Position,
		type Node,
		type Edge
	} from '@xyflow/svelte';
	import '@xyflow/svelte/dist/style.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { graphNodes, graphEdges, graphLoading } from '$lib/stores/graph';
	import { docTypes } from '$lib/stores/docs';
	import { SvelteSet } from 'svelte/reactivity';
	import { setContext, untrack } from 'svelte';
	import { writable } from 'svelte/store';
	import DocNode from '$lib/components/graph/DocNode.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { getEdgeStyle, EXCLUDED_RELATIONS, CATEGORIES } from '$lib/config/relations';

	const nodeTypes = { doc: DocNode };

	const HIDDEN_BY_DEFAULT = new Set(['deprecated', 'superseded']);

	const NODE_W = 280;
	const NODE_H = 48;

	const statusColors: Record<string, { bg: string; text: string; border: string }> = {
		identified: { bg: 'bg-blue-100', text: 'text-blue-800', border: 'border-blue-200' },
		proposed: { bg: 'bg-amber-100', text: 'text-amber-800', border: 'border-amber-200' },
		draft: { bg: 'bg-amber-100', text: 'text-amber-800', border: 'border-amber-200' },
		validating: { bg: 'bg-amber-100', text: 'text-amber-800', border: 'border-amber-200' },
		pursuing: { bg: 'bg-amber-100', text: 'text-amber-800', border: 'border-amber-200' },
		active: { bg: 'bg-emerald-100', text: 'text-emerald-800', border: 'border-emerald-200' },
		accepted: { bg: 'bg-emerald-100', text: 'text-emerald-800', border: 'border-emerald-200' },
		resolved: { bg: 'bg-emerald-100', text: 'text-emerald-800', border: 'border-emerald-200' },
		deprecated: { bg: 'bg-red-100', text: 'text-red-800', border: 'border-red-200' },
		superseded: { bg: 'bg-red-100', text: 'text-red-800', border: 'border-red-200' },
		completed: { bg: 'bg-slate-100', text: 'text-slate-700', border: 'border-slate-200' }
	};
	const defaultStatusColor = { bg: 'bg-gray-100', text: 'text-gray-700', border: 'border-gray-200' };

	let hiddenStatuses = new SvelteSet(HIDDEN_BY_DEFAULT);
	let hiddenTypes = new SvelteSet<string>();

	// ?focus=<id> — restrict the graph to that doc's depth-2 neighborhood
	let focusId = $state(page.url.searchParams.get('focus')?.toUpperCase() ?? null);

	const typeGroupColors: Record<string, { fill: string; border: string }> = {
		adr: { fill: 'rgba(59,130,246,0.06)', border: 'rgba(59,130,246,0.15)' },
		opp: { fill: 'rgba(16,185,129,0.06)', border: 'rgba(16,185,129,0.15)' },
		pol: { fill: 'rgba(147,51,234,0.06)', border: 'rgba(147,51,234,0.15)' },
		inc: { fill: 'rgba(239,68,68,0.06)', border: 'rgba(239,68,68,0.15)' },
		spec: { fill: 'rgba(245,158,11,0.06)', border: 'rgba(245,158,11,0.15)' }
	};
	const defaultGroupColor = { fill: 'rgba(148,163,184,0.06)', border: 'rgba(148,163,184,0.15)' };

	const typeFilterColors: Record<string, { bg: string; text: string; border: string }> = {
		adr: { bg: 'bg-blue-100', text: 'text-blue-800', border: 'border-blue-200' },
		opp: { bg: 'bg-emerald-100', text: 'text-emerald-800', border: 'border-emerald-200' },
		pol: { bg: 'bg-purple-100', text: 'text-purple-800', border: 'border-purple-200' },
		inc: { bg: 'bg-red-100', text: 'text-red-800', border: 'border-red-200' },
		spec: { bg: 'bg-amber-100', text: 'text-amber-800', border: 'border-amber-200' }
	};
	const defaultTypeColor = { bg: 'bg-gray-100', text: 'text-gray-700', border: 'border-gray-200' };
	let flowNodes = $state<Node[]>([]);
	let flowEdges = $state<Edge[]>([]);
	let hoveredNodeId = $state<string | null>(null);

	// Share hover state with DocNode via context
	const hoveredStore = writable<Set<string>>(new Set());
	setContext('graphHighlight', hoveredStore);

	/** Extract numeric suffix from ID for sorting (e.g. ADR-003 → 3) */
	function idNum(id: string): number {
		const m = id.match(/(\d+)$/);
		return m ? parseInt(m[1], 10) : 0;
	}

	/** Status display order: blue → yellow → green → red → gray */
	const statusOrder: Record<string, number> = {
		identified: 0,
		proposed: 1, draft: 2, validating: 3, pursuing: 4,
		active: 5, accepted: 6, resolved: 7,
		deprecated: 8, superseded: 9,
		completed: 10
	};

	/** Collect unique statuses from graph data, ordered by color group */
	const allStatuses = $derived(
		[...new Set($graphNodes.map((n) => n.status?.toLowerCase()).filter(Boolean))]
			.sort((a, b) => (statusOrder[a] ?? 99) - (statusOrder[b] ?? 99))
	);

	function toggleStatus(status: string) {
		if (hiddenStatuses.has(status)) hiddenStatuses.delete(status);
		else hiddenStatuses.add(status);
	}

	function toggleType(type: string) {
		if (hiddenTypes.has(type)) hiddenTypes.delete(type);
		else hiddenTypes.add(type);
	}

	/** Collect unique doc types from graph data */
	const allTypes = $derived(
		[...new Set($graphNodes.map((n) => n.type?.toLowerCase()).filter(Boolean))].sort()
	);

	$effect(() => {
		const rawNodes = $graphNodes;
		const rawEdges = $graphEdges;
		const hidden = hiddenStatuses;
		const hTypes = hiddenTypes;

		const hideRelated = rawNodes.length > 10;
		const filteredEdges = rawEdges.filter((e) =>
			!EXCLUDED_RELATIONS.has(e.relation) && !(hideRelated && e.relation === 'related')
		);

		const connectedIds = new Set<string>();
		for (const e of filteredEdges) {
			connectedIds.add(e.source);
			connectedIds.add(e.target);
		}

		// Sort by ID number so oldest appear first (dagre respects input order for ranking)
		let connectedNodes = rawNodes
			.filter((n) => connectedIds.has(n.id))
			.sort((a, b) => idNum(a.id) - idNum(b.id));

		// ?focus= restricts to the depth-2 neighborhood of one doc
		if (focusId) {
			const adjacency = new Map<string, Set<string>>();
			for (const e of filteredEdges) {
				if (!adjacency.has(e.source)) adjacency.set(e.source, new Set());
				if (!adjacency.has(e.target)) adjacency.set(e.target, new Set());
				adjacency.get(e.source)!.add(e.target);
				adjacency.get(e.target)!.add(e.source);
			}
			const included = new Set<string>([focusId]);
			let frontier = [focusId];
			for (let d = 0; d < 2; d++) {
				const next: string[] = [];
				for (const id of frontier) {
					for (const neighbor of adjacency.get(id) ?? []) {
						if (!included.has(neighbor)) {
							included.add(neighbor);
							next.push(neighbor);
						}
					}
				}
				frontier = next;
			}
			connectedNodes = connectedNodes.filter((n) => included.has(n.id));
		}

		// Apply status + type filters — only layout visible nodes
		const visibleNodes = connectedNodes.filter(
			(n) => !hidden.has(n.status?.toLowerCase()) && !hTypes.has(n.type?.toLowerCase())
		);
		const visibleIds = new Set(visibleNodes.map((n) => n.id));
		const visibleEdges = filteredEdges.filter(
			(e) => visibleIds.has(e.source) && visibleIds.has(e.target)
		);

		if (visibleNodes.length === 0) {
			flowNodes = [];
			flowEdges = [];
			return;
		}

		const g = new dagre.graphlib.Graph();
		g.setGraph({ rankdir: 'TB', ranksep: 70, nodesep: 24, edgesep: 15, ranker: 'tight-tree' });
		g.setDefaultEdgeLabel(() => ({}));

		for (const n of visibleNodes) {
			g.setNode(n.id, { width: NODE_W, height: NODE_H });
		}
		for (const e of visibleEdges) {
			g.setEdge(e.source, e.target);
		}

		dagre.layout(g);

		// Build group overlay nodes by doc type
		const typeGroups = new Map<string, { minX: number; minY: number; maxX: number; maxY: number }>();
		for (const n of visibleNodes) {
			const pos = g.node(n.id);
			const type = (n.type as string)?.toLowerCase() ?? '';
			if (!type) continue;
			const x1 = pos.x - NODE_W / 2;
			const y1 = pos.y - NODE_H / 2;
			const x2 = pos.x + NODE_W / 2;
			const y2 = pos.y + NODE_H / 2;
			const existing = typeGroups.get(type);
			if (existing) {
				existing.minX = Math.min(existing.minX, x1);
				existing.minY = Math.min(existing.minY, y1);
				existing.maxX = Math.max(existing.maxX, x2);
				existing.maxY = Math.max(existing.maxY, y2);
			} else {
				typeGroups.set(type, { minX: x1, minY: y1, maxX: x2, maxY: y2 });
			}
		}

		const pad = 20;
		const groupNodes: Node[] = [...typeGroups.entries()].map(([type, bounds]) => {
			const gc = typeGroupColors[type] ?? defaultGroupColor;
			return {
				id: `__group_${type}`,
				type: 'group',
				position: { x: bounds.minX - pad, y: bounds.minY - pad },
				style: `width: ${bounds.maxX - bounds.minX + pad * 2}px; height: ${bounds.maxY - bounds.minY + pad * 2}px; background: ${gc.fill}; border: 1px solid ${gc.border}; border-radius: 12px;`,
				selectable: false,
				draggable: false,
				data: {}
			};
		});

		const docNodes: Node[] = visibleNodes.map((n) => {
			const pos = g.node(n.id);
			return {
				id: n.id,
				type: 'doc',
				position: { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 },
				data: {
					label: n.id,
					title: n.title,
					docType: n.type,
					status: n.status
				},
				sourcePosition: Position.Bottom,
				targetPosition: Position.Top
			};
		});

		// Group nodes first so they render behind doc nodes
		flowNodes = [...groupNodes, ...docNodes];

		flowEdges = visibleEdges.map((e) => {
			const es = getEdgeStyle(e.relation);
			const dashPart = es.strokeDasharray ? ` stroke-dasharray: ${es.strokeDasharray};` : '';
			const opacityPart = es.opacity < 1 ? ` opacity: ${es.opacity};` : '';
			return {
				id: `${e.source}-${e.relation}-${e.target}`,
				source: e.source,
				target: e.target,
				type: 'default',
				animated: false,
				data: { relation: e.relation },
				style: `stroke: ${es.color}; stroke-width: ${es.strokeWidth};${dashPart}${opacityPart}`,
				markerEnd: es.markerEnd ? { type: 'arrowclosed' as const, color: es.color } : undefined
			};
		});
	});

	// Update edge styles and node highlight context when hover changes
	$effect(() => {
		const hId = hoveredNodeId;
		// Read edges without tracking to avoid infinite loop
		const edges = untrack(() => flowEdges);

		if (!hId) {
			hoveredStore.set(new Set());
			flowEdges = edges.map((e) => {
				const relation = (e.data?.relation as string) ?? '';
				const es = getEdgeStyle(relation);
				const dashPart = es.strokeDasharray ? ` stroke-dasharray: ${es.strokeDasharray};` : '';
				const opacityPart = es.opacity < 1 ? ` opacity: ${es.opacity};` : '';
				return {
					...e,
					label: undefined,
					labelStyle: undefined,
					labelBgStyle: undefined,
					style: `stroke: ${es.color}; stroke-width: ${es.strokeWidth};${dashPart}${opacityPart}`
				};
			});
			return;
		}

		// Compute connected node IDs
		const hlNodes = new Set<string>([hId]);
		const hlEdgeIds = new Set<string>();
		for (const e of edges) {
			if (e.source === hId || e.target === hId) {
				hlNodes.add(e.source);
				hlNodes.add(e.target);
				hlEdgeIds.add(e.id);
			}
		}

		hoveredStore.set(hlNodes);

		// Dim unrelated edges, show labels only on connected edges
		flowEdges = edges.map((e) => {
			const relation = (e.data?.relation as string) ?? '';
			const es = getEdgeStyle(relation);
			const connected = hlEdgeIds.has(e.id);
			const dashPart = es.strokeDasharray ? ` stroke-dasharray: ${es.strokeDasharray};` : '';
			const opacityPart = !connected ? ' opacity: 0.1;' : (es.opacity < 1 ? ` opacity: ${es.opacity};` : '');
			return {
				...e,
				label: connected ? relation : undefined,
				labelStyle: connected ? `fill: ${es.color}; font-size: 10px; font-weight: 500;` : undefined,
				labelBgStyle: connected ? 'fill: var(--card); fill-opacity: 0.85;' : undefined,
				style: `stroke: ${es.color}; stroke-width: ${es.strokeWidth};${dashPart}${opacityPart}`
			};
		});
	});

	function typeFolder(type: string): string {
		return $docTypes[type]?.folder ?? type;
	}

	function onNodePointerEnter({ node }: { node: Node; event: PointerEvent }) {
		hoveredNodeId = node.id;
	}

	function onNodePointerLeave() {
		hoveredNodeId = null;
	}

	function onNodeClick({ node }: { node: Node; event: MouseEvent | TouchEvent }) {
		const docType = node.data.docType as string;
		goto(`/${typeFolder(docType)}/${node.id.toLowerCase()}`);
	}


</script>

<svelte:head>
	<title>Dependency Graph</title>
</svelte:head>

<div class="mx-auto max-w-7xl">
	<div class="mb-4 flex items-center gap-3">
		<h1 class="text-2xl font-bold text-foreground">Dependency Graph</h1>
		{#if focusId}
			<Button
				variant="secondary"
				size="sm"
				class="h-7 rounded-full px-2.5 text-[11px]"
				onclick={() => {
					focusId = null;
					goto('/graph', { replaceState: true });
				}}
			>
				Focused on {focusId} ✕
			</Button>
		{/if}
	</div>

	<div class="relative rounded-xl border bg-card shadow-sm" style="height: calc(100vh - 10rem);">
		<!-- Control bar -->
		<div class="absolute top-0 left-0 right-0 z-20 flex items-center gap-3 px-3 py-2 bg-card/80 backdrop-blur-sm border-b rounded-t-xl">
			{#if allTypes.length > 1}
				<div class="flex flex-wrap gap-1.5">
					{#each allTypes as type (type)}
						{@const colors = typeFilterColors[type] ?? defaultTypeColor}
						{@const isHidden = hiddenTypes.has(type)}
						<Button
							variant={isHidden ? 'outline' : 'secondary'}
							size="sm"
							onclick={() => toggleType(type)}
							class="h-7 px-2.5 text-[11px] rounded-full uppercase {isHidden
								? 'text-muted-foreground line-through opacity-60'
								: `${colors.bg} ${colors.text} ${colors.border}`}"
						>
							{type}
						</Button>
					{/each}
				</div>
				<div class="w-px h-5 bg-border"></div>
			{/if}
			{#if allStatuses.length > 0}
				<div class="flex flex-wrap gap-1.5">
					{#each allStatuses as status (status)}
						{@const colors = statusColors[status] ?? defaultStatusColor}
						{@const isHidden = hiddenStatuses.has(status)}
						<Button
							variant={isHidden ? 'outline' : 'secondary'}
							size="sm"
							onclick={() => toggleStatus(status)}
							class="h-7 px-2.5 text-[11px] rounded-full {isHidden
								? 'text-muted-foreground line-through opacity-60'
								: `${colors.bg} ${colors.text} ${colors.border}`}"
						>
							{status}
						</Button>
					{/each}
				</div>
			{/if}
			<div class="ml-auto flex gap-3 text-xs text-muted-foreground">
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
		</div>

		<SvelteFlow
			bind:nodes={flowNodes}
			bind:edges={flowEdges}
			{nodeTypes}
			fitView
			maxZoom={1.5}
			nodesDraggable={true}
			nodesConnectable={false}
			elementsSelectable={true}
			onnodeclick={onNodeClick}
			onnodepointerenter={onNodePointerEnter}
			onnodepointerleave={onNodePointerLeave}
		>
			<Background variant={BackgroundVariant.Dots} gap={16} size={1} />
			<Controls />
		</SvelteFlow>

		{#if $graphLoading}
			<div class="absolute inset-0 z-10 flex items-center justify-center bg-card/90">
				<div class="text-muted-foreground">Loading graph...</div>
			</div>
		{:else if flowNodes.length === 0}
			<div class="absolute inset-0 z-10 flex items-center justify-center bg-card">
				<div class="text-muted-foreground">No connected documents found.</div>
			</div>
		{/if}
	</div>
</div>
