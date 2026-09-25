<script lang="ts">
	const ORIGIN = 'https://decisiongraph.dev';

	let {
		title = 'Decision Graph: decisions as code',
		description = 'Track architecture decisions, policies, opportunities, incidents, specs and processes as schema-validated Markdown in your git repo. CLI, web UI and AI agent skills included.',
		path = '/'
	}: { title?: string; description?: string; path?: string } = $props();

	const url = $derived(ORIGIN + path);
	const image = `${ORIGIN}/screenshot-graph.png`;
	const jsonLd = $derived(
		JSON.stringify({
			'@context': 'https://schema.org',
			'@type': 'SoftwareApplication',
			name: 'Decision Graph',
			applicationCategory: 'DeveloperApplication',
			operatingSystem: 'macOS, Linux, Windows',
			description,
			url,
			license: 'https://www.gnu.org/licenses/agpl-3.0.html',
			codeRepository: 'https://github.com/decisiongraph/dg',
			offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' }
		})
	);
</script>

<svelte:head>
	<title>{title}</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={url} />
	<meta property="og:type" content="website" />
	<meta property="og:site_name" content="Decision Graph" />
	<meta property="og:title" content={title} />
	<meta property="og:description" content={description} />
	<meta property="og:url" content={url} />
	<meta property="og:image" content={image} />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="800" />
	<meta property="og:image:alt" content="Decision Graph web UI showing a graph of linked decisions" />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={title} />
	<meta name="twitter:description" content={description} />
	<meta name="twitter:image" content={image} />
	{@html `<script type="application/ld+json">${jsonLd.replace(/</g, '\\u003c')}</script>`}
</svelte:head>
