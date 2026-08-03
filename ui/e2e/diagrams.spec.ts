import { test, expect } from './fixtures';

test('proc Process Flow mermaid diagram renders to SVG', async ({ page }) => {
	const msgs: string[] = [];
	page.on('console', (m) => msgs.push(`[${m.type()}] ${m.text().slice(0, 300)}`));
	page.on('pageerror', (e) => msgs.push(`[pageerror] ${e.message.slice(0, 300)}`));

	await page.goto('/processes/proc-001');
	try {
		await page
			.locator('svg[id^="mermaid-"]')
			.first()
			.waitFor({ state: 'visible', timeout: 20_000 });
	} catch {
		const body = await page
			.evaluate(() => document.body.innerText.slice(0, 300).replace(/\n/g, '|'))
			.catch(() => '(unavailable)');
		throw new Error(`mermaid svg missing.\nbody: ${body}\nconsole:\n${msgs.join('\n')}`);
	}
	await expect(page.getByText('Mermaid diagram failed to render')).toHaveCount(0);
});

test('d2 diagram on incident page renders or degrades gracefully', async ({ page, request }) => {
	test.setTimeout(60_000);
	const msgs: string[] = [];
	page.on('console', (m) => msgs.push(`[${m.type()}] ${m.text().slice(0, 300)}`));
	page.on('pageerror', (e) => msgs.push(`[pageerror] ${e.message.slice(0, 300)}`));
	page.on('requestfailed', (r) =>
		msgs.push(`[requestfailed] ${r.url().slice(-80)} ${r.failure()?.errorText}`)
	);
	const pending = new Set<string>();
	page.on('request', (r) => pending.add(r.url()));
	page.on('requestfinished', (r) => pending.delete(r.url()));
	page.on('requestfailed', (r) => pending.delete(r.url()));
	// The d2 renderer is fetched from a CDN at site-gen time; offline builds ship
	// without it. Only assert SVG rendering when the bundle is present.
	const bundle = await request.get('/data/d2/d2-browser.js');
	await page.goto('/incidents/inc-001');

	if (bundle.ok()) {
		// d2 compiles in WASM in the browser — generous timeout under parallel load
		try {
			await page.locator('svg.d2-svg').first().waitFor({ state: 'visible', timeout: 45_000 });
		} catch {
			const body = await page
				.evaluate(() => document.body.innerText.slice(0, 300).replace(/\n/g, '|'))
				.catch(() => '(unavailable)');
			throw new Error(
				`d2 svg missing.\nbody: ${body}\npending requests: ${[...pending].join(', ')}\nconsole:\n${msgs.join('\n')}`
			);
		}
	} else {
		// Graceful degradation: error panel with the diagram source, no crash
		await expect(page.getByText('D2 diagram failed to render')).toBeVisible({ timeout: 20_000 });
		await expect(page.getByText('Show diagram source')).toBeVisible();
	}
});
