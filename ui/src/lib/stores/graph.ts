import { writable, derived } from 'svelte/store';
import { base } from '$app/paths';
import type { GraphData, GraphNode, GraphEdge } from '$lib/types';

export const graphData = writable<GraphData | null>(null);
export const graphLoading = writable(true);

export const graphNodes = derived(graphData, ($data) => $data?.nodes ?? []);
export const graphEdges = derived(graphData, ($data) => $data?.edges ?? []);

let loaded = false;

export async function loadGraph() {
	if (loaded) return;
	loaded = true;
	graphLoading.set(true);
	try {
		const res = await fetch(`${base}/data/graph.json`);
		if (!res.ok) throw new Error(`Failed to load graph: ${res.status}`);
		const data: GraphData = await res.json();
		graphData.set(data);
	} catch (e) {
		console.error('Failed to load graph data:', e);
	} finally {
		graphLoading.set(false);
	}
}

/** Map type prefix to color for SvelteFlow nodes */
export const typeColors: Record<string, string> = {
	adr: '#3b82f6',
	opp: '#10b981',
	pol: '#8b5cf6',
	inc: '#ef4444',
	spec: '#f59e0b'
};
