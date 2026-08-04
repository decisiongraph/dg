import { test, expect } from './fixtures';

const DATA_FILES = [
	'docs.json',
	'graph.json',
	'nav.json',
	'org.json',
	'roadmap.json',
	'search-index.json',
	'services.json',
	'site-meta.json'
];

test('data files are served as JSON', async ({ request }) => {
	for (const file of DATA_FILES) {
		const res = await request.get(`/data/${file}`);
		expect(res.status(), `/data/${file}`).toBe(200);
		expect(res.headers()['content-type'], `/data/${file}`).toContain('application/json');
	}
});

test('immutable assets get long-lived cache headers', async ({ request }) => {
	const html = await (await request.get('/')).text();
	const match = html.match(/\/_app\/immutable\/[^"']+\.js/);
	expect(match, 'index.html should reference immutable assets').toBeTruthy();

	const res = await request.get(match![0]);
	expect(res.status()).toBe(200);
	expect(res.headers()['cache-control']).toContain('immutable');
});

test('deep links and dotted SPA routes fall back to the SPA shell', async ({ request }) => {
	for (const route of ['/architecture/adr-001', '/org/users/john.doe']) {
		const res = await request.get(route);
		expect(res.status(), route).toBe(200);
		expect(res.headers()['content-type'], route).toContain('text/html');
		expect(await res.text()).toContain('<html');
	}
});

test('percent-encoded traversal attempts are rejected', async ({ request }) => {
	const res = await request.get('/..%2F..%2FCargo.toml');
	expect(res.status()).toBe(404);
});

test('unknown static assets 404 instead of returning the SPA shell', async ({ request }) => {
	const res = await request.get('/no-such-image.png');
	expect(res.status()).toBe(404);
});
