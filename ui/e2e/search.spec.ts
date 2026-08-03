import { test, expect } from './fixtures';

test('command palette finds a spec and navigates', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('button', { name: /Search/ }).click();

	const input = page.getByPlaceholder('Search...');
	await expect(input).toBeVisible();
	await input.fill('websocket');

	// spec-001 "Real-time Document Sync via WebSocket" should surface
	const hit = page.getByRole('option').filter({ hasText: /WebSocket/i }).first();
	await expect(hit).toBeVisible();
	await hit.click();
	await expect(page).toHaveURL(/\/specifications\/spec-001/);
});
