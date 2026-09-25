import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		fs: {
			// +page.server.ts reads crates/dg-schemas/schema.kdl at build time
			allow: ['..']
		}
	}
});
