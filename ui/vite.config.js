import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	build: {
		minify: false
	},
	plugins: [
		tailwindcss(/** @type {any} */ ({
			exclude: ['**/node_modules/@xyflow/**']
		})),
		sveltekit()
	],
	resolve: {
		dedupe: ['svelte']
	},
	server: {
		proxy: {
			'/data': 'http://127.0.0.1:10050'
		}
	}
});
