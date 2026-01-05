import { writable } from 'svelte/store';
import { base } from '$app/paths';

export interface JiraConfig {
	prefix: string;
	url: string;
}

export interface SiteMeta {
	title: string;
	readme_html?: string;
	logo_url?: string;
	jira?: JiraConfig[];
	edit_url_prefix?: string;
	is_local_dev?: boolean;
	source_path?: string;
}

export const siteMeta = writable<SiteMeta>({ title: 'Documentation' });
export const siteMetaLoading = writable(true);
export const readmeHtml = writable<string | undefined>(undefined);
export const readmeSourcePath = writable<string | undefined>(undefined);
export const readmeLoading = writable(true);

let loaded = false;
let readmeLoaded = false;

export async function loadSiteMeta() {
	if (loaded) return;
	loaded = true;
	siteMetaLoading.set(true);
	try {
		const res = await fetch(`${base}/data/site-meta.json`);
		if (!res.ok) throw new Error(`Failed to load site-meta: ${res.status}`);
		const data: SiteMeta = await res.json();
		siteMeta.set(data);
	} catch (e) {
		console.error('Failed to load site meta:', e);
	} finally {
		siteMetaLoading.set(false);
	}
}

export async function loadReadme() {
	if (readmeLoaded) return;
	readmeLoaded = true;
	readmeLoading.set(true);
	try {
		const res = await fetch(`${base}/data/readme.json`);
		if (!res.ok) throw new Error(`Failed to load readme: ${res.status}`);
		const data = await res.json();
		readmeHtml.set(data.html ?? undefined);
		readmeSourcePath.set(data.source_path ?? undefined);
	} catch (e) {
		console.error('Failed to load readme:', e);
	} finally {
		readmeLoading.set(false);
	}
}
