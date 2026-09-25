<script lang="ts">
	import { page } from '$app/state';
</script>

<svelte:head>
	<title>{page.status === 404 ? 'Page not found' : 'Error'} | Decision Graph</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<section class="mx-auto max-w-2xl px-4 py-24 text-center">
	<h1 class="mb-4 text-5xl font-bold">{page.status}</h1>
	<p class="mb-8 text-fg-2">
		{page.status === 404 ? "This page doesn't exist." : page.error?.message}
	</p>
	<a href="/" class="btn btn-primary">Back to home</a>
</section>
