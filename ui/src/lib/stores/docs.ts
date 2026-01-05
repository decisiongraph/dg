import { writable, derived } from 'svelte/store';
import { base } from '$app/paths';
import type { DocsData, DocEntry } from '$lib/types';

export const docsData = writable<DocsData | null>(null);
export const docsLoading = writable(true);
export const docsError = writable<string | null>(null);

export const allDocs = derived(docsData, ($data) => $data?.docs ?? []);
export const docTypes = derived(docsData, ($data) => $data?.types ?? {});

export function docsByType(type: string) {
	return derived(allDocs, ($docs) => $docs.filter((d) => d.type === type));
}

export function docById(id: string) {
	return derived(allDocs, ($docs) => $docs.find((d) => d.id.toLowerCase() === id.toLowerCase()));
}

let loaded = false;

export async function loadDocs() {
	if (loaded) return;
	loaded = true;
	docsLoading.set(true);
	try {
		const res = await fetch(`${base}/data/docs.json`);
		if (!res.ok) throw new Error(`Failed to load docs: ${res.status}`);
		const data: DocsData = await res.json();
		docsData.set(data);
		docsError.set(null);
	} catch (e) {
		docsError.set(e instanceof Error ? e.message : 'Unknown error');
	} finally {
		docsLoading.set(false);
	}
}
