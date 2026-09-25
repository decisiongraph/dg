import { readdirSync } from 'node:fs';
import { resolve } from 'node:path';

export const prerender = true;
export const trailingSlash = 'never';

const ORIGIN = 'https://decisiongraph.dev';

export const GET = () => {
	const demos = readdirSync(resolve('demos'), { withFileTypes: true })
		.filter((d) => d.isDirectory())
		.map((d) => d.name)
		.sort();
	const urls = [
		{ loc: '/', changefreq: 'weekly', priority: '1.0' },
		...demos.map((d) => ({ loc: `/demo/${d}/`, changefreq: 'monthly', priority: '0.8' }))
	];
	const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls
	.map(
		(u) =>
			`  <url>\n    <loc>${ORIGIN}${u.loc}</loc>\n    <changefreq>${u.changefreq}</changefreq>\n    <priority>${u.priority}</priority>\n  </url>`
	)
	.join('\n')}
</urlset>
`;
	return new Response(body, { headers: { 'Content-Type': 'application/xml' } });
};
