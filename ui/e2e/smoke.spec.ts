import { test, expect } from './fixtures';

const STATIC_ROUTES = [
	'/',
	'/roadmap',
	'/kanban',
	'/graph',
	'/onboarding',
	'/org',
	'/org/teams',
	'/org/users'
];

test('static routes render without page errors', async ({ page }) => {
	const pageErrors: string[] = [];
	page.on('pageerror', (err) => pageErrors.push(err.message));

	for (const route of STATIC_ROUTES) {
		await page.goto(route);
		await expect(page.locator('h1, h2').first()).toBeVisible();
	}

	expect(pageErrors, `page errors: ${pageErrors.join('\n')}`).toHaveLength(0);
});

test('doc-type list routes derived from docs.json render', async ({ page, request }) => {
	const { types } = await (await request.get('/data/docs.json')).json();
	const pageErrors: string[] = [];
	page.on('pageerror', (err) => pageErrors.push(err.message));

	for (const info of Object.values(types) as { folder: string; display: string }[]) {
		await page.goto(`/${info.folder}`);
		await expect(page.locator('h1').first()).toContainText(info.display);
	}

	expect(pageErrors, `page errors: ${pageErrors.join('\n')}`).toHaveLength(0);
});
