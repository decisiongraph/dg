import { loadGraph } from '$lib/stores/graph';

export const load = async () => {
	await loadGraph();
	return {};
};
