/**
 * `dg site` output must work when hosted under a subpath (issue #4), e.g.
 * https://decisiongraph.dev/demo/pied-piper/. Builds the example site into
 * <tmp>/demo/sub/ and serves <tmp> with a plain static server (no SPA
 * fallback, no trailing-slash redirects), then checks that every internal
 * link, request and navigation stays under /demo/sub/.
 */
import { test, expect, type Page } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import type { AddressInfo } from 'node:net';
import path from 'node:path';
import { copyExampleProject, requireDgBinary, saveSharedCache } from './fixtures';

const BASE = '/demo/sub';

const MIME: Record<string, string> = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript',
	'.css': 'text/css',
	'.json': 'application/json',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.jpg': 'image/jpeg'
};

let project: string;
let hostRoot: string;
let server: http.Server;
let origin: string;

test.beforeAll(async () => {
	project = copyExampleProject();
	hostRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'dg-e2e-host-'));
	const build = spawnSync(
		requireDgBinary(),
		['site', '--no-git', '--root', project, '-o', path.join(hostRoot, BASE)],
		{ encoding: 'utf8' }
	);
	expect(build.status, build.stderr).toBe(0);
	saveSharedCache(project);

	// Dumb static file server: file, or directory index.html. Extensionless
	// misses under the subpath get the root shell (Netlify-style SPA fallback).
	server = http.createServer((req, res) => {
		const urlPath = decodeURIComponent(new URL(req.url ?? '/', 'http://x').pathname);
		let file = path.join(hostRoot, path.normalize(urlPath));
		if (!file.startsWith(hostRoot)) return res.writeHead(404).end();
		if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
		if (!fs.existsSync(file) && urlPath.startsWith(`${BASE}/`) && !path.extname(urlPath)) {
			file = path.join(hostRoot, BASE, 'index.html');
		}
		if (!fs.existsSync(file) || !fs.statSync(file).isFile()) return res.writeHead(404).end();
		res.writeHead(200, { 'content-type': MIME[path.extname(file)] ?? 'application/octet-stream' });
		fs.createReadStream(file).pipe(res);
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
});

test.afterAll(() => {
	server?.close();
	for (const dir of [project, hostRoot]) if (dir) fs.rmSync(dir, { recursive: true, force: true });
});

/** Record same-origin requests that escape the subpath (assets, data, fetches). */
function trackEscapes(page: Page): string[] {
	const escaped: string[] = [];
	page.on('request', (req) => {
		const url = new URL(req.url());
		// blob: URLs (e.g. web workers) share the origin but aren't server requests
		if (url.protocol !== 'http:' || url.origin !== origin) return;
		if (!url.pathname.startsWith(`${BASE}/`)) escaped.push(url.pathname);
	});
	return escaped;
}

/** Every root-relative href/src rendered on the page must carry the subpath. */
async function expectLinksUnderBase(page: Page) {
	const paths = await page.evaluate(() =>
		[...document.querySelectorAll('a[href], img[src]')]
			.map((el) => el.getAttribute('href') ?? el.getAttribute('src') ?? '')
			.filter((v) => v.startsWith('/') && !v.startsWith('//'))
	);
	expect(paths.length).toBeGreaterThan(0);
	expect(paths.filter((p) => !p.startsWith(`${BASE}/`))).toEqual([]);
}

test('index + sidebar nav stay under the subpath', async ({ page }) => {
	const escaped = trackEscapes(page);
	await page.goto(`${origin}${BASE}/`);
	const nav = page.locator(`a[href="${BASE}/architecture/"]`).first();
	await expect(nav).toBeVisible({ timeout: 15_000 });
	await expectLinksUnderBase(page);

	await nav.click();
	await expect(page).toHaveURL(new RegExp(`^${origin}${BASE}/architecture/?$`));
	await expect(page.locator(`a[href="${BASE}/architecture/adr-001"]`).first()).toBeVisible();
	expect(escaped).toEqual([]);
});

test('doc deep link: content refs + mentions stay under the subpath', async ({ page }) => {
	const escaped = trackEscapes(page);
	// No trailing slash: shells must not depend on relative URL resolution
	await page.goto(`${origin}${BASE}/architecture/adr-002`);
	const ref = page.locator(`p a[href="${BASE}/opportunities/opp-002"]`).first();
	await expect(ref).toBeVisible({ timeout: 15_000 });
	await expectLinksUnderBase(page);

	await ref.click();
	await expect(page).toHaveURL(`${origin}${BASE}/opportunities/opp-002`);
	await expect(page.locator('h1').first()).toContainText('Migrate from AWS');
	await expectLinksUnderBase(page);

	// @mentions in doc tables link to user pages
	await page.goto(`${origin}${BASE}/opportunities/opp-002/`);
	const mention = page.locator(`td a[href="${BASE}/org/users/eve"]`).first();
	await expect(mention).toBeVisible();
	await expectLinksUnderBase(page);

	await mention.click();
	await expect(page).toHaveURL(`${origin}${BASE}/org/users/eve`);
	expect(escaped).toEqual([]);
});

test('root shell served as SPA fallback still finds the subpath', async ({ page }) => {
	const escaped = trackEscapes(page);
	// adr-999 has no per-route shell, so the server falls back to the root index.html
	await page.goto(`${origin}${BASE}/architecture/adr-999`);
	await expect(page.locator(`a[href="${BASE}/architecture/"]`).first()).toBeVisible({
		timeout: 15_000
	});
	await expectLinksUnderBase(page);
	expect(escaped).toEqual([]);
});

test('graph node navigation stays under the subpath', async ({ page }) => {
	const escaped = trackEscapes(page);
	await page.goto(`${origin}${BASE}/graph/`);
	const node = page.locator('.svelte-flow__node-doc').first();
	await expect(node).toBeVisible({ timeout: 20_000 });
	await node.click();
	await expect(page).toHaveURL(
		new RegExp(`^${origin}${BASE}/(architecture|policies|opportunities|specifications|incidents|processes)/`),
		{ timeout: 10_000 }
	);
	await expect(page.locator('h1').first()).toBeVisible();
	expect(escaped).toEqual([]);
});
