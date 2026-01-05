/** Matches Rust JSON output from site/data.rs */

export interface TypeInfo {
	display: string;
	folder: string;
}

export interface Backlink {
	id: string;
	relation: string;
	title: string;
}

export interface DocEntry {
	id: string;
	type: string;
	title: string;
	status: string;
	author?: string;
	date?: string;
	tags?: string[];
	severity?: string;
	priority?: string;
	category?: string;
	meta?: Record<string, string>;
	people?: Record<string, string[]>;
	body_html: string;
	links: Record<string, string[] | null>;
	backlinks: Backlink[];
	source_path?: string;
}

export interface DocsData {
	types: Record<string, TypeInfo>;
	docs: DocEntry[];
}

export interface GraphNode {
	id: string;
	type: string;
	title: string;
	status: string;
}

export interface GraphEdge {
	source: string;
	target: string;
	relation: string;
}

export interface GraphData {
	nodes: GraphNode[];
	edges: GraphEdge[];
}

export interface TeamDef {
	name: string;
	lead?: string;
	members: string[];
	parent?: string;
	children?: string[];
	org?: string;
	status: string;
	description?: string;
	body_html?: string;
	extra?: Record<string, string>;
}

export interface UserDef {
	name: string;
	title?: string;
	email?: string;
	teams: string[];
	status: string;
	kind: string;
	org?: string;
	avatar_url?: string;
}

export interface OrgDef {
	name: string;
	parent?: string;
	children: string[];
}

export interface OrgData {
	teams: Record<string, TeamDef>;
	users: Record<string, UserDef>;
	orgs: Record<string, OrgDef>;
}

export interface NavItem {
	label: string;
	href?: string;
	children?: NavItem[];
}

export interface SearchEntry {
	id: string;
	title: string;
	body: string;
	type: string;
	subtitle?: string;
	href?: string;
}

export interface RoadmapItem {
	id: string;
	title: string;
	status: string;
	date?: string;
	effort?: string;
	priority?: string;
	owner?: string;
	team?: string;
	quarter?: string;
}

export interface RoadmapData {
	items: RoadmapItem[];
	html?: string;
	generated_at?: string;
}

export interface Assignment {
	doc_id: string;
	doc_type: string;
	doc_title: string;
	role: string;
	description?: string;
	status?: string;
	due_date?: string;
	section?: string;
}

export interface AssignmentsData {
	users: Record<string, Assignment[]>;
}

export interface ServiceLanguage {
	name: string;
	percentage: number;
}

export interface EolWarning {
	product: string;
	version: string;
	eol_date?: string;
}

export interface ServiceEntry {
	slug: string;
	name: string;
	kind: string;
	status: string;
	owner: string;
	owner_team?: string;
	description: string;
	readme_path: string;
	body_html: string;
	primary_language: string;
	languages: ServiceLanguage[];
	frameworks: string[];
	framework_versions?: [string, string][];
	deployment_platform?: string;
	database?: string;
	lines_of_code?: number;
	dependencies_count?: number;
	repo_size?: string;
	language_version?: string;
	created_at?: string;
	commit_count?: number;
	last_commit_at?: string;
	has_linter: boolean;
	linter_tool?: string;
	has_tests: boolean;
	test_framework?: string;
	dev_commands?: {
		setup?: string;
		build?: string;
		test?: string;
		run?: string;
		lint?: string;
	};
	eol_warnings?: EolWarning[];
}

export interface ServicesData {
	services: ServiceEntry[];
	devicon_urls: Record<string, string>;
}

// ── Code references ─────────────────────────────────────────────────

export interface CodeRef {
	file: string;
	line: number;
	text: string;
}

export interface CommitRef {
	sha: string;
	subject: string;
	date: string;
	author: string;
	body_context?: string;
}

export interface DocCodeRefs {
	code: CodeRef[];
	commits: CommitRef[];
}

export interface CodeRefsData {
	refs: Record<string, DocCodeRefs>;
	commit_url_prefix?: string;
	file_url_prefix?: string;
}

// ── Schema metadata (tooltips) ──────────────────────────────────────

export interface SchemaEnumValue {
	name: string;
	description?: string;
	transitions?: string[];
}

export interface SchemaFieldInfo {
	description?: string;
	values: SchemaEnumValue[];
}

export interface SchemaTypeInfo {
	description?: string;
	statuses: SchemaEnumValue[];
	fields: Record<string, SchemaFieldInfo>;
}

export interface SchemaData {
	types: Record<string, SchemaTypeInfo>;
}
