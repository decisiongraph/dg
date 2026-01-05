import { writable } from 'svelte/store';
import { base } from '$app/paths';
import type { SchemaData } from '$lib/types';

export const schemaData = writable<SchemaData | null>(null);

let loaded = false;

export async function loadSchema() {
	if (loaded) return;
	loaded = true;
	try {
		const res = await fetch(`${base}/data/schema.json`);
		if (!res.ok) return;
		const data: SchemaData = await res.json();
		schemaData.set(data);
	} catch {
		// Schema tooltips are non-critical — fail silently
	}
}
