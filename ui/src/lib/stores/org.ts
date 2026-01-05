import { writable } from 'svelte/store';
import { base } from '$app/paths';
import type { OrgData } from '$lib/types';

export const orgData = writable<OrgData | null>(null);
export const orgLoading = writable(true);

let loaded = false;

export async function loadOrg() {
	if (loaded) return;
	loaded = true;
	orgLoading.set(true);
	try {
		const res = await fetch(`${base}/data/org.json`);
		if (!res.ok) throw new Error(`Failed to load org: ${res.status}`);
		const data: OrgData = await res.json();
		orgData.set(data);
	} catch (e) {
		console.error('Failed to load org data:', e);
	} finally {
		orgLoading.set(false);
	}
}
