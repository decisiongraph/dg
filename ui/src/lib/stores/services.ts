import { writable, derived } from 'svelte/store';
import { base } from '$app/paths';
import type { ServicesData, ServiceEntry } from '$lib/types';

export const servicesData = writable<ServicesData | null>(null);
export const servicesLoading = writable(true);

let loaded = false;

export async function loadServices() {
	if (loaded) return;
	loaded = true;
	servicesLoading.set(true);
	try {
		const res = await fetch(`${base}/data/services.json`);
		if (!res.ok) throw new Error(`Failed to load services: ${res.status}`);
		const data: ServicesData = await res.json();
		servicesData.set(data);
	} catch (e) {
		console.error('Failed to load services data:', e);
	} finally {
		servicesLoading.set(false);
	}
}

export const allServices = derived(servicesData, ($d) => $d?.services ?? []);
export const deviconUrls = derived(servicesData, ($d) => $d?.devicon_urls ?? {});

/** Look up a service by slug. */
export function serviceBySlug(services: ServiceEntry[], slug: string): ServiceEntry | undefined {
	return services.find((s) => s.slug === slug);
}

/** Look up CDN URL for a technology name. */
export function deviconUrl(urls: Record<string, string>, name: string): string | undefined {
	return urls[name.toLowerCase()];
}
