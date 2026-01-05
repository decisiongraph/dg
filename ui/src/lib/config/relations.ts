/**
 * Relation display config — maps internal relation names to human-readable
 * labels and visual categories for sidebar + graph.
 *
 * Internal schema names are unchanged; this is display-only.
 */

export type RelationCategory = 'drivers' | 'impact' | 'conflict' | 'context';

interface RelationLabel {
	label: string;
	category: RelationCategory;
}

/** Outgoing links (from this doc's frontmatter) — label describes what the *other* doc experiences */
export const OUTGOING_LABELS: Record<string, RelationLabel> = {
	enables: { label: 'Enabled by', category: 'impact' },
	triggers: { label: 'Triggered by', category: 'impact' },
	supersedes: { label: 'Superseded by', category: 'impact' },
	implements: { label: 'Implemented by', category: 'drivers' },
	depends_on: { label: 'Depended on by', category: 'drivers' },
	conflicts_with: { label: 'Conflicts with', category: 'conflict' },
	related: { label: 'Related to', category: 'context' }
};

/** Incoming backlinks (keyed by the source doc's outgoing relation) — label describes what the *other* doc does */
export const INCOMING_LABELS: Record<string, RelationLabel> = {
	enables: { label: 'Enabled', category: 'drivers' },
	triggers: { label: 'Triggered', category: 'drivers' },
	supersedes: { label: 'Superseded', category: 'drivers' },
	implements: { label: 'Implements', category: 'impact' },
	depends_on: { label: 'Dependency of', category: 'impact' },
	conflicts_with: { label: 'Conflicts with', category: 'conflict' },
	related: { label: 'Related to', category: 'context' }
};

interface CategoryDef {
	label: string;
	color: string;
	/** CSS stroke-dasharray, empty string = solid */
	dasharray: string;
	opacity: number;
	strokeWidth: number;
	arrow: boolean;
}

export const CATEGORIES: Record<RelationCategory, CategoryDef> = {
	drivers: {
		label: 'Upstream Drivers',
		color: '#475569', // slate-600
		dasharray: '',
		opacity: 1,
		strokeWidth: 2,
		arrow: true
	},
	impact: {
		label: 'Downstream Impact',
		color: '#2563eb', // blue-600
		dasharray: '',
		opacity: 1,
		strokeWidth: 2,
		arrow: true
	},
	conflict: {
		label: 'Conflict',
		color: '#f43f5e', // rose-500
		dasharray: '6 4',
		opacity: 1,
		strokeWidth: 2,
		arrow: false
	},
	context: {
		label: 'Context',
		color: '#94a3b8', // slate-400
		dasharray: '',
		opacity: 0.5,
		strokeWidth: 1.5,
		arrow: false
	}
};

/** Get the visual category for a relation name (outgoing direction) */
export function getEdgeCategory(relation: string): RelationCategory {
	return OUTGOING_LABELS[relation]?.category ?? 'context';
}

interface EdgeStyle {
	color: string;
	strokeDasharray: string;
	opacity: number;
	strokeWidth: number;
	markerEnd: boolean;
}

/** Get SVG edge styling for a relation */
export function getEdgeStyle(relation: string): EdgeStyle {
	const cat = CATEGORIES[getEdgeCategory(relation)];
	return {
		color: cat.color,
		strokeDasharray: cat.dasharray,
		opacity: cat.opacity,
		strokeWidth: cat.strokeWidth,
		markerEnd: cat.arrow
	};
}

/** All inverse relation names that should be excluded from graph edges */
export const EXCLUDED_RELATIONS = new Set([
	'enabled_by',
	'triggered_by',
	'superseded_by',
	'dependency_of',
	'implemented_by'
]);

/** Sidebar section definitions in display order — uses hex colors for inline styles
 *  (Tailwind can't purge dynamic classes from .ts config files) */
export const SIDEBAR_SECTIONS: { key: RelationCategory; label: string; accentColor: string; badgeBg: string; badgeText: string }[] = [
	{ key: 'drivers', label: 'Upstream Drivers', accentColor: '#64748b', badgeBg: '#f1f5f9', badgeText: '#334155' },
	{ key: 'impact', label: 'Downstream Impact', accentColor: '#2563eb', badgeBg: '#dbeafe', badgeText: '#1d4ed8' },
	{ key: 'conflict', label: 'Conflict', accentColor: '#f43f5e', badgeBg: '#ffe4e6', badgeText: '#be123c' },
	{ key: 'context', label: 'Context', accentColor: '#cbd5e1', badgeBg: '#f1f5f9', badgeText: '#94a3b8' }
];
