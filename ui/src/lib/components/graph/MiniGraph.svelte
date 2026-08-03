<script lang="ts">
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
	import { goto } from '$app/navigation';
	import { graphNodes, graphEdges, loadGraph } from '$lib/stores/graph';
	import { docTypes } from '$lib/stores/docs';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import DocNode from '$lib/components/graph/DocNode.svelte';
	import { getEdgeStyle, EXCLUDED_RELATIONS } from '$lib/config/relations';

	interface Props {
		/** Doc ID whose neighborhood to show (e.g. "SPEC-001") */
		focusId: string;
		/** BFS depth in both directions */
		depth?: number;
		height?: string;
	}

	let { focusId, depth = 2, height = '300px' }: Props = $props();

	loadGraph();

	const nodeTypes = { doc: DocNode };
	const NODE_W = 280;
	const NODE_H = 48;

	// DocNode reads this context for hover highlighting on the full graph page
	setContext('graphHighlight', writable<Set<string>>(new Set()));

	let flowNodes = $state<Node[]>([]);
	let flowEdges = $state<Edge[]>([]);

	$effect(() => {
		const edges = $graphEdges.filter((e) => !EXCLUDED_RELATIONS.has(e.relation));
		const adjacency = new Map<string, Set<string>>();
		for (const e of edges) {
			if (!adjacency.has(e.source)) adjacency.set(e.source, new Set());
			if (!adjacency.has(e.target)) adjacency.set(e.target, new Set());
			adjacency.get(e.source)!.add(e.target);
			adjacency.get(e.target)!.add(e.source);
		}

		const focus = focusId.toUpperCase();
		const included = new Set<string>([focus]);
		let frontier = [focus];
		for (let d = 0; d < depth; d++) {
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

		const nodes = $graphNodes.filter((n) => included.has(n.id));
		const subEdges = edges.filter((e) => included.has(e.source) && included.has(e.target));
		if (nodes.length <= 1) {
			flowNodes = [];
			flowEdges = [];
			return;
		}

		const g = new dagre.graphlib.Graph();
		g.setGraph({ rankdir: 'TB', ranksep: 50, nodesep: 20, edgesep: 12, ranker: 'tight-tree' });
		g.setDefaultEdgeLabel(() => ({}));
		for (const n of nodes) {
			g.setNode(n.id, { width: NODE_W, height: NODE_H });
		}
		for (const e of subEdges) {
			g.setEdge(e.source, e.target);
		}
		dagre.layout(g);

		flowNodes = nodes.map((n) => {
			const pos = g.node(n.id);
			return {
				id: n.id,
				type: 'doc',
				position: { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 },
				class: n.id === focus ? 'minigraph-focus' : '',
				draggable: false,
				data: { label: n.id, title: n.title, docType: n.type, status: n.status },
				sourcePosition: Position.Bottom,
				targetPosition: Position.Top
			};
		});

		flowEdges = subEdges.map((e) => {
			const es = getEdgeStyle(e.relation);
			const dashPart = es.strokeDasharray ? ` stroke-dasharray: ${es.strokeDasharray};` : '';
			const opacityPart = es.opacity < 1 ? ` opacity: ${es.opacity};` : '';
			return {
				id: `${e.source}-${e.relation}-${e.target}`,
				source: e.source,
				target: e.target,
				type: 'default',
				animated: false,
				label: e.relation,
				labelStyle: `fill: ${es.color}; font-size: 9px;`,
				labelBgStyle: 'fill: var(--card); fill-opacity: 0.85;',
				style: `stroke: ${es.color}; stroke-width: ${es.strokeWidth};${dashPart}${opacityPart}`,
				markerEnd: es.markerEnd ? { type: 'arrowclosed' as const, color: es.color } : undefined
			};
		});
	});

	function onNodeClick({ node }: { node: Node }) {
		if (node.id === focusId.toUpperCase()) return;
		const docType = node.data.docType as string;
		const folder = $docTypes[docType]?.folder ?? docType;
		goto(`/${folder}/${node.id.toLowerCase()}`);
	}
</script>

{#if flowNodes.length > 1}
	<div class="rounded-lg border bg-card overflow-hidden" style="height: {height}" data-testid="mini-graph">
		<SvelteFlow
			bind:nodes={flowNodes}
			bind:edges={flowEdges}
			{nodeTypes}
			fitView
			maxZoom={1.2}
			nodesDraggable={false}
			nodesConnectable={false}
			elementsSelectable={false}
			onnodeclick={onNodeClick}
		>
			<Background variant={BackgroundVariant.Dots} gap={16} size={1} />
		</SvelteFlow>
	</div>
{/if}

<style>
	:global(.svelte-flow__node.minigraph-focus > div) {
		box-shadow: 0 0 0 2px var(--primary);
	}
</style>
