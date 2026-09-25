import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { demoStats } from '$lib/demos';
import { summarizeSchema } from '$lib/schema';
import type { PageServerLoad } from './$types';

export const prerender = true;

// Vite runs from website/; the built-in dg schema is the single source of
// truth for the record types listed on the page (see issue #5).
const SCHEMA_PATH = resolve('..', 'crates', 'dg-schemas', 'schema.kdl');
const DEMOS_DIR = resolve('demos');

function stats(name: string) {
	const s = demoStats(resolve(DEMOS_DIR, name));
	if (s.docs === 0) throw new Error(`demo ${name}: no documents found in ${DEMOS_DIR}/${name}/docs`);
	return s;
}

export const load: PageServerLoad = () => {
	if (!existsSync(SCHEMA_PATH)) throw new Error(`schema not found: ${SCHEMA_PATH}`);
	return {
		schema: summarizeSchema(readFileSync(SCHEMA_PATH, 'utf8')),
		demos: { 'pied-piper': stats('pied-piper'), microsoft: stats('microsoft') }
	};
};
