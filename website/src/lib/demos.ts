import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { parseKdl } from './kdl';

export interface DemoStats {
	docs: number;
	users: number;
	teams: number;
	/** Document counts per ID prefix, e.g. { ADR: 5, INC: 4 } */
	byPrefix: Record<string, number>;
}

function walkMarkdown(dir: string): string[] {
	if (!existsSync(dir)) return [];
	return readdirSync(dir).flatMap((entry) => {
		if (entry.startsWith('.')) return [];
		const path = join(dir, entry);
		if (statSync(path).isDirectory()) return walkMarkdown(path);
		return entry.endsWith('.md') ? [path] : [];
	});
}

/** Count docs, users and teams of a dg project (`website/demos/<name>`). */
export function demoStats(root: string): DemoStats {
	const byPrefix: Record<string, number> = {};
	let docs = 0;
	for (const file of walkMarkdown(join(root, 'docs'))) {
		const m = /^([a-z]+)-\d+/i.exec(file.split('/').pop() ?? '');
		if (!m) continue;
		docs++;
		const prefix = m[1].toUpperCase();
		byPrefix[prefix] = (byPrefix[prefix] ?? 0) + 1;
	}
	const orgPath = join(root, '.dg', 'org.kdl');
	const org = existsSync(orgPath) ? parseKdl(readFileSync(orgPath, 'utf8')) : [];
	return {
		docs,
		users: org.filter((n) => n.name === 'user').length,
		teams: org.filter((n) => n.name === 'team').length,
		byPrefix
	};
}
