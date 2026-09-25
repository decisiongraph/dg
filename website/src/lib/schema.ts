import { parseKdl, type KdlNode } from './kdl';

export interface RecordType {
	/** Type key, e.g. `adr` */
	name: string;
	/** ID prefix shown on docs, e.g. `ADR` */
	prefix: string;
	description: string;
	folder: string;
	aliases: string[];
	/** Required top-level sections, in schema order */
	requiredSections: string[];
	/** Values of the `status` enum field, if any */
	statuses: string[];
}

export interface Relation {
	name: string;
	inverse: string | null;
	description: string;
}

export interface SchemaSummary {
	/** Numbered document types (ADR-001, ...) */
	records: RecordType[];
	/** Singleton types such as README checks (one file per folder, no IDs) */
	singletons: RecordType[];
	relations: Relation[];
}

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

function toRecordType(node: KdlNode): RecordType {
	const name = str(node.args[0]);
	const status = node.children.find((c) => c.name === 'field' && c.args[0] === 'status');
	return {
		name,
		prefix: name.toUpperCase(),
		description: str(node.props.description),
		folder: str(node.props.folder),
		aliases: node.children.filter((c) => c.name === 'alias').flatMap((c) => c.args.map(str)),
		requiredSections: node.children
			.filter((c) => c.name === 'section' && c.props.required === true)
			.map((c) => str(c.args[0])),
		statuses:
			status?.children.find((c) => c.name === 'values')?.args.map(str).filter(Boolean) ?? []
	};
}

export function summarizeSchema(kdl: string): SchemaSummary {
	const nodes = parseKdl(kdl);
	const types = nodes.filter((n) => n.name === 'type' && typeof n.args[0] === 'string');
	if (types.length === 0) throw new Error('schema.kdl: no `type` nodes found');
	return {
		records: types.filter((t) => t.props.singleton !== true).map(toRecordType),
		singletons: types.filter((t) => t.props.singleton === true).map(toRecordType),
		relations: nodes
			.filter((n) => n.name === 'relation')
			.map((n) => ({
				name: str(n.args[0]),
				inverse: str(n.props.inverse) || null,
				description: str(n.props.description)
			}))
	};
}
