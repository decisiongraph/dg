import { writable } from 'svelte/store';
import { base } from '$app/paths';
import type { CodeRefsData, DocCodeRefs } from '$lib/types';

export const codeRefsData = writable<CodeRefsData | null>(null);
export const codeRefsLoading = writable(true);

let loaded = false;

export async function loadCodeRefs() {
	if (loaded) return;
	loaded = true;
	codeRefsLoading.set(true);
	try {
		const res = await fetch(`${base}/data/code-refs.json`);
		if (!res.ok) throw new Error(`Failed to load code refs: ${res.status}`);
		const data: CodeRefsData = await res.json();
		codeRefsData.set(data);
	} catch (e) {
		console.error('Failed to load code refs data:', e);
	} finally {
		codeRefsLoading.set(false);
	}
}

/** Get code refs for a single document ID. */
export function codeRefsForDoc(
	data: CodeRefsData | null,
	docId: string
): DocCodeRefs | null {
	if (!data) return null;
	return data.refs[docId.toUpperCase()] ?? null;
}
