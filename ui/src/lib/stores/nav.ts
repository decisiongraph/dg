import { writable } from 'svelte/store';
import { base } from '$app/paths';
import type { NavItem } from '$lib/types';

export const navData = writable<NavItem[]>([]);
export const navLoading = writable(true);

let loaded = false;

export async function loadNav() {
	if (loaded) return;
	loaded = true;
	navLoading.set(true);
	try {
		const res = await fetch(`${base}/data/nav.json`);
		if (!res.ok) throw new Error(`Failed to load nav: ${res.status}`);
		const data: NavItem[] = await res.json();
		navData.set(data);
	} catch (e) {
		console.error('Failed to load nav data:', e);
	} finally {
		navLoading.set(false);
	}
}
