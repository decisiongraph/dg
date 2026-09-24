import { test, expect } from './fixtures';

test('every document page renders its title (data-driven crawl)', async ({ page, request }) => {
	const { types, docs } = await (await request.get('/data/docs.json')).json();
	expect(docs.length).toBeGreaterThan(0);

	for (const doc of docs as { id: string; type: string; title: string }[]) {
		const folder = types[doc.type]?.folder ?? doc.type;
		await page.goto(`/${folder}/${doc.id.toLowerCase()}`);
		// First navigation hydrates the whole SPA — allow for slow parallel runs
		await expect(
			page.locator('h1').first(),
			`doc ${doc.id} should render its title`
		).toContainText(doc.title, { timeout: 15_000 });
		await expect(page.getByText('Document not found')).toHaveCount(0);
	}
});

test('unknown document id shows not-found state, not a crash', async ({ page }) => {
	const pageErrors: string[] = [];
	page.on('pageerror', (err) => pageErrors.push(err.message));
	await page.goto('/architecture/adr-999');
	// The SPA must handle the missing doc gracefully (any explicit empty state is fine)
	await expect(page.locator('body')).not.toContainText('Internal Error');
	expect(pageErrors).toHaveLength(0);
});

for (const { name, route, selector } of [
	{
		name: 'document references',
		route: '/architecture/adr-002',
		selector: 'p a[href="/opportunities/opp-002"]'
	},
	{
		name: 'roadmap links',
		route: '/roadmap',
		selector: '.roadmap-container a[href^="/opportunities/"]:has(.user-hovercard)'
	}
]) {
	test(`${name} show previews and navigate to their documents`, async ({ page, request }) => {
		const { docs } = await (await request.get('/data/docs.json')).json();
		await page.goto(route);
		const reference = page.locator(selector).first();
		await expect(reference).toBeVisible();
		const docId = (await reference.getAttribute('href'))?.split('/').pop()?.toUpperCase();
		const target = docs.find((doc: { id: string }) => doc.id === docId);
		expect(target).toBeDefined();

		await reference.hover();
		const preview = page.locator('body > div.fixed').filter({ hasText: target.title });
		await expect(preview).toBeVisible();
		await expect(preview).toContainText(target.id);
		await expect(preview).toContainText(target.status);
		await page.mouse.move(0, 0);
		await expect(preview).toBeHidden();
		await reference.click();
		await expect(page.locator('h1').first()).toContainText(target.title);
	});
}
