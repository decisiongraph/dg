import { writable } from 'svelte/store';
import { base } from '$app/paths';
import type { AssignmentsData, Assignment } from '$lib/types';

export const assignmentsData = writable<AssignmentsData | null>(null);
export const assignmentsLoading = writable(true);

let loaded = false;

export async function loadAssignments() {
	if (loaded) return;
	loaded = true;
	assignmentsLoading.set(true);
	try {
		const res = await fetch(`${base}/data/assignments.json`);
		if (!res.ok) throw new Error(`Failed to load assignments: ${res.status}`);
		const data: AssignmentsData = await res.json();
		assignmentsData.set(data);
	} catch (e) {
		console.error('Failed to load assignments data:', e);
	} finally {
		assignmentsLoading.set(false);
	}
}

/** Get assignments for a single user handle. */
export function assignmentsForHandle(
	data: AssignmentsData | null,
	handle: string
): Assignment[] {
	if (!data) return [];
	return data.users[handle.toLowerCase()] ?? [];
}

/** Get assignments for multiple handles (e.g. all team members). */
export function assignmentsForHandles(
	data: AssignmentsData | null,
	handles: string[]
): Assignment[] {
	if (!data) return [];
	return handles.flatMap((h) => data.users[h.toLowerCase()] ?? []);
}
