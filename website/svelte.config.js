import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		// Fully prerendered. 404.html is what Cloudflare Pages serves for
		// unknown paths (it renders +error.svelte). /demo/* is written into
		// build/ by build.sh after `vite build`.
		adapter: adapter({ pages: 'build', assets: 'build', fallback: '404.html', strict: true }),
		prerender: {
			// Demo links point at dg-generated sites that only exist after build.sh
			handleHttpError: ({ path, message }) => {
				if (path.startsWith('/demo/')) return;
				throw new Error(message);
			}
		}
	}
};

export default config;
