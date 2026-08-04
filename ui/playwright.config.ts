import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
	testDir: './e2e',
	fullyParallel: true,
	// Each worker spawns its own `dg serve` (site build + mermaid/d2 WASM in the
	// browser); too many workers starve page hydration into timeouts.
	workers: 2,
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 2 : 0,
	reporter: process.env.CI ? [['github'], ['list']] : 'list',
	use: {
		trace: 'on-first-retry'
	},
	projects: [
		{
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] }
		}
	]
});
