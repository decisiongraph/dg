import { base } from '$app/paths';
import type { AssignmentsData, DocsData } from '$lib/types';

export const load = async () => {
	const [assignmentsRes, docsRes] = await Promise.all([
		fetch(`${base}/data/assignments.json`),
		fetch(`${base}/data/docs.json`)
	]);

	const assignments: AssignmentsData | null = assignmentsRes.ok
		? await assignmentsRes.json()
		: null;
	const docs: DocsData | null = docsRes.ok ? await docsRes.json() : null;

	return { assignments, docs };
};
