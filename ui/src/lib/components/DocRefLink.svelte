<script lang="ts">
	import { allDocs, docTypes } from '$lib/stores/docs';
	import * as HoverCard from '$lib/components/ui/hover-card/index.js';
	import StatusBadge from './StatusBadge.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		refId: string;
		children: Snippet;
	}

	let { refId, children }: Props = $props();

	const refType = $derived(refId.split('-')[0]?.toLowerCase());
	const folder = $derived($docTypes[refType]?.folder ?? refType);
	const href = $derived(`/${folder}/${refId.toLowerCase()}`);
	const refDoc = $derived($allDocs.find((d) => d.id.toLowerCase() === refId.toLowerCase()));

	/** Extract first heading + first paragraph from body HTML */
	function firstSection(html: string): { heading?: string; body: string } {
		const headingRe = /<h[1-6][^>]*>(.*?)<\/h[1-6]>/;
		const hMatch = html.match(headingRe);
		const strip = (s: string) => s.replace(/<[^>]+>/g, '').replace(/\s+/g, ' ').trim();

		if (hMatch) {
			const heading = strip(hMatch[1]);
			const after = html.slice(html.indexOf(hMatch[0]) + hMatch[0].length);
			const nextH = after.search(/<h[1-6]/);
			const sectionHtml = nextH >= 0 ? after.slice(0, nextH) : after;
			// First <p> in section
			const pMatch = sectionHtml.match(/<p[^>]*>([\s\S]*?)<\/p>/);
			const body = pMatch ? strip(pMatch[1]) : strip(sectionHtml);
			return { heading, body };
		}

		// No heading — first paragraph
		const pMatch = html.match(/<p[^>]*>([\s\S]*?)<\/p>/);
		return { body: pMatch ? strip(pMatch[1]) : strip(html) };
	}

	const preview = $derived(refDoc?.body_html ? firstSection(refDoc.body_html) : undefined);
</script>

<HoverCard.Root openDelay={300} closeDelay={100}>
	<HoverCard.Trigger {href}>
		{@render children()}
	</HoverCard.Trigger>
	{#if refDoc}
		<HoverCard.Content class="w-72">
			<div class="space-y-2">
				<div class="flex items-center gap-2">
					<span class="font-mono text-xs text-muted-foreground">{refDoc.id}</span>
					{#if refDoc.status}
						<StatusBadge status={refDoc.status} docType={refDoc.type} />
					{/if}
				</div>
				<p class="text-sm font-medium leading-tight">{refDoc.title}</p>
				{#if preview?.body}
					<p class="text-xs text-muted-foreground leading-relaxed">
						{#if preview.heading}<strong class="text-foreground/80">{preview.heading}:</strong>{' '}{/if}{preview.body}
					</p>
				{/if}
				{#if refDoc.author || refDoc.date}
					<div class="flex gap-3 text-[10px] text-muted-foreground pt-1 border-t">
						{#if refDoc.author}<span>@{refDoc.author}</span>{/if}
						{#if refDoc.date}<span>{refDoc.date}</span>{/if}
					</div>
				{/if}
			</div>
		</HoverCard.Content>
	{/if}
</HoverCard.Root>
