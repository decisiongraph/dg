import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { parseKdl } from './kdl';
import { summarizeSchema } from './schema';

describe('parseKdl', () => {
	test('args, props, children, keywords, comments', () => {
		const nodes = parseKdl(`
			// comment
			type "adr" description="Arch \\"quoted\\"" n=1_000 singleton=#true {
				alias "architecture" tech /* inline */ ; field "x" required=true
				/- section "skipped"
				section "S" { content min-paragraphs=1 }
			}
			relation "r" \\
				inverse=#"raw\\"#
		`);
		expect(nodes).toHaveLength(2);
		const [t, r] = nodes;
		expect(t.args).toEqual(['adr']);
		expect(t.props).toEqual({ description: 'Arch "quoted"', n: 1000, singleton: true });
		expect(t.children.map((c) => c.name)).toEqual(['alias', 'field', 'section']);
		expect(t.children[0].args).toEqual(['architecture', 'tech']);
		expect(t.children[1].props.required).toBe(true);
		expect(t.children[2].children[0].props['min-paragraphs']).toBe(1);
		expect(r.props.inverse).toBe('raw\\');
	});
});

describe('summarizeSchema', () => {
	test('built-in schema', () => {
		const kdl = readFileSync(
			new URL('../../../crates/dg-schemas/schema.kdl', import.meta.url),
			'utf8'
		);
		const s = summarizeSchema(kdl);
		expect(s.records.map((r) => r.prefix)).toContain('ADR');
		expect(s.records.every((r) => r.description && r.folder)).toBe(true);
		expect(s.singletons.map((r) => r.name)).toContain('readme');
		expect(s.relations.map((r) => r.name)).toContain('supersedes');
		const adr = s.records.find((r) => r.name === 'adr');
		expect(adr?.requiredSections).toEqual(['Context', 'Decision', 'Consequences']);
		expect(adr?.aliases).toContain('architecture');
	});
});
