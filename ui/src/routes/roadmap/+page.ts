import { base } from '$app/paths';
import type { RoadmapData } from '$lib/types';

export const load = async () => {
	try {
		const res = await fetch(`${base}/data/roadmap.json`);
		if (!res.ok) return { roadmap: null };
		const data: RoadmapData = await res.json();
		return { roadmap: data };
	} catch {
		return { roadmap: null };
	}
};
