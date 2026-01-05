import Fuse from 'fuse.js';
import { writable, derived } from 'svelte/store';
import { base } from '$app/paths';
import type { SearchEntry } from '$lib/types';

export const searchIndex = writable<SearchEntry[]>([]);
export const searchQuery = writable('');
export const searchOpen = writable(false);

const fuseInstance = derived(searchIndex, ($index) => {
	return new Fuse($index, {
		keys: [
			{ name: 'id', weight: 2 },
			{ name: 'title', weight: 1.5 },
			{ name: 'subtitle', weight: 0.8 },
			{ name: 'body', weight: 0.5 }
		],
		threshold: 0.35,
		ignoreLocation: true,
		minMatchCharLength: 2
	});
});

export const searchResults = derived([fuseInstance, searchQuery], ([$fuse, $query]) => {
	if (!$query || $query.length < 2) return [];
	return $fuse.search($query).map((r) => r.item).slice(0, 12);
});

let loaded = false;

export async function loadSearchIndex() {
	if (loaded) return;
	loaded = true;
	try {
		const res = await fetch(`${base}/data/search-index.json`);
		if (!res.ok) return;
		const data: SearchEntry[] = await res.json();
		searchIndex.set(data);
	} catch {
		// Search is optional, fail silently
	}
}
