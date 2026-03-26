<script lang="ts">
	import { mount } from 'svelte';
	import { isDark } from '$lib/stores/theme';
	import { enrichContentRefs, enrichExistingDocLinks } from '$lib/actions/content-refs';
	import { enrichUserMentions } from '$lib/actions/user-mentions';
	import { orgData } from '$lib/stores/org';
	import CodeCopyButton from '$lib/components/CodeCopyButton.svelte';

	interface Props {
		html: string;
		class?: string;
	}

	let { html, class: className = '' }: Props = $props();

	interface Segment {
		type: 'html' | 'mermaid' | 'd2';
		content: string;
	}

	function decodeEntities(s: string): string {
		return s
			.replace(/&lt;/g, '<')
			.replace(/&gt;/g, '>')
			.replace(/&amp;/g, '&')
			.replace(/&quot;/g, '"')
			.replace(/&#39;/g, "'");
	}

	const segments: Segment[] = $derived.by(() => {
		const result: Segment[] = [];
		const regex =
			/<pre><code class="language-(mermaid|d2)">([\s\S]*?)<\/code><\/pre>/g;
		let lastIndex = 0;
		let match;

		while ((match = regex.exec(html)) !== null) {
			if (match.index > lastIndex) {
				result.push({ type: 'html', content: html.slice(lastIndex, match.index) });
			}
			const lang = match[1] as 'mermaid' | 'd2';
			const raw = decodeEntities(match[2]);
			result.push({ type: lang, content: raw });
			lastIndex = match.index + match[0].length;
		}

		if (lastIndex < html.length) {
			result.push({ type: 'html', content: html.slice(lastIndex) });
		}

		return result;
	});

	const SPINNER_HTML = `<div class="flex flex-col items-center gap-2 py-4">
		<svg class="animate-spin h-6 w-6 text-muted-foreground" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
			<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
			<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
		</svg>
		<span class="text-xs text-muted-foreground">Loading diagram...</span>
	</div>`;

	/** Move subgraph labels to top-left so arrows don't overlap them. */
	function fixMermaidClusterLabels(container: HTMLElement) {
		const svg = container.querySelector('svg');
		if (!svg) return;
		const clusters = svg.querySelectorAll('.cluster.default, .cluster');
		for (const cluster of clusters) {
			const rect = cluster.querySelector('rect');
			const label = cluster.querySelector('.cluster-label');
			if (!rect || !label) continue;
			const rx = parseFloat(rect.getAttribute('x') || '0');
			const ry = parseFloat(rect.getAttribute('y') || '0');
			label.setAttribute('transform', `translate(${rx + 8}, ${ry})`);
		}
	}

	/** Expand Mermaid SVG viewBox by a small margin so node text isn't clipped. */
	function padMermaidViewBox(container: HTMLElement) {
		const svg = container.querySelector('svg');
		if (!svg) return;
		const vb = svg.getAttribute('viewBox');
		if (!vb) return;
		const parts = vb.split(/[\s,]+/).map(Number);
		if (parts.length !== 4 || parts.some(isNaN)) return;
		const pad = 10;
		svg.setAttribute('viewBox', `${parts[0] - pad} ${parts[1] - pad} ${parts[2] + pad * 2} ${parts[3] + pad * 2}`);
	}

	function renderMermaid(el: HTMLElement, source: string) {
		void renderMermaidAsync(el, source);
	}

	async function renderMermaidAsync(el: HTMLElement, source: string) {
		el.innerHTML = SPINNER_HTML;
		try {
			const dark = $isDark;
			const mermaid = (await import('mermaid')).default;
			mermaid.initialize({ startOnLoad: false, theme: dark ? 'dark' : 'default' });
			const id = `mermaid-${Math.random().toString(36).slice(2, 9)}`;
			const { svg } = await mermaid.render(id, source);
			el.innerHTML = svg;
			fixMermaidClusterLabels(el);
			padMermaidViewBox(el);
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			console.error('Mermaid render failed:', err);
			el.innerHTML = `<div class="space-y-3">
				<div class="flex items-start gap-2 rounded-md border border-red-200 bg-red-50 dark:bg-red-950/30 dark:border-red-800 px-3 py-2">
					<span class="text-red-500 text-sm leading-none mt-0.5">⚠</span>
					<div class="min-w-0">
						<p class="text-sm font-medium text-red-700 dark:text-red-400">Mermaid diagram failed to render</p>
						<p class="text-xs text-red-600/80 dark:text-red-400/70 mt-0.5 break-all">${escapeHtml(msg)}</p>
					</div>
				</div>
				<details class="group">
					<summary class="cursor-pointer text-xs text-muted-foreground hover:text-foreground select-none">Show diagram source</summary>
					<pre class="mt-2 rounded-md bg-muted p-3 text-xs overflow-x-auto"><code>${escapeHtml(source)}</code></pre>
				</details>
			</div>`;
		}
	}

	let d2Module: { D2: any } | null = null;
	let d2LoadError: string | null = null;

	async function loadD2(): Promise<{ D2: any }> {
		if (d2Module) return d2Module;
		if (d2LoadError) throw new Error(d2LoadError);
		try {
			const load = new Function('url', 'return import(url)');
			d2Module = await load('/data/d2/d2-browser.js');
			return d2Module!;
		} catch (err) {
			d2LoadError = err instanceof Error ? err.message : String(err);
			throw err;
		}
	}

	/**
	 * D2 generates dark mode CSS using @media screen and (prefers-color-scheme:dark){...}.
	 * Our app uses class-based dark mode (.dark on <html>), so we rewrite
	 * the media queries into class-based selectors that respond to our toggle.
	 * Uses brace-counting instead of regex to reliably find the closing brace.
	 */
	function patchD2DarkCss(svg: string): string {
		const marker = 'prefers-color-scheme';
		let pos = 0;
		let result = svg;
		while (true) {
			const mediaStart = result.indexOf('@media', pos);
			if (mediaStart === -1) break;
			const markerIdx = result.indexOf(marker, mediaStart);
			if (markerIdx === -1) break;
			// Find the opening brace of this @media block
			const openBrace = result.indexOf('{', markerIdx);
			if (openBrace === -1) break;
			// Brace-count to find the matching close
			let depth = 1;
			let i = openBrace + 1;
			while (i < result.length && depth > 0) {
				if (result[i] === '{') depth++;
				else if (result[i] === '}') depth--;
				i++;
			}
			if (depth !== 0) break;
			const closeBrace = i - 1;
			const inner = result.substring(openBrace + 1, closeBrace);
			// Rewrite each CSS rule to be scoped under html.dark
			const rewritten = inner.replace(
				/([^{}]+)\{([^{}]*)\}/g,
				(_, selector: string, body: string) =>
					`html.dark ${selector.trim()} { ${body.trim()} }\n`
			);
			result = result.substring(0, mediaStart) + rewritten + result.substring(closeBrace + 1);
			pos = mediaStart + rewritten.length;
		}
		return result;
	}

	/** Prepend dark-theme-id config to D2 source so the SVG includes dark mode CSS. */
	function withDarkTheme(source: string): string {
		// If the source already has vars/d2-config, don't double-inject
		if (source.includes('dark-theme-id')) return source;
		return `vars: {\n  d2-config: {\n    dark-theme-id: 200\n  }\n}\n${source}`;
	}

	/**
	 * Post-process an HTML container to add +/- markers on consequence lists.
	 * Finds headings containing "positive/pros/benefits" → green + bullets,
	 * and "negative/cons/risks/drawbacks" → red - bullets.
	 */
	function annotateConsequences(el: HTMLElement) {
		const positiveRe = /\b(positive|pros|benefits|advantages)\b/i;
		const negativeRe = /\b(negative|cons|risks|drawbacks|disadvantages)\b/i;

		const headings = el.querySelectorAll('h1, h2, h3, h4, h5, h6');
		for (const heading of headings) {
			const text = heading.textContent ?? '';
			let cls = '';
			if (positiveRe.test(text)) cls = 'consequence-positive';
			else if (negativeRe.test(text)) cls = 'consequence-negative';
			if (!cls) continue;

			// Walk siblings after the heading until next heading or end
			let sibling = heading.nextElementSibling;
			while (sibling && !/^H[1-6]$/.test(sibling.tagName)) {
				if (sibling.tagName === 'UL') {
					sibling.classList.add(cls);
				}
				sibling = sibling.nextElementSibling;
			}
		}
	}

	/**
	 * Add a "#" row-number column to tables that follow specific headings
	 * (e.g. "Action Items", "Requirements").
	 */
	function numberTableRows(el: HTMLElement) {
		const targetRe = /\b(action items|requirements)\b/i;
		const headings = el.querySelectorAll('h1, h2, h3, h4, h5, h6');
		for (const heading of headings) {
			if (!targetRe.test(heading.textContent ?? '')) continue;
			let sibling = heading.nextElementSibling;
			while (sibling && !/^H[1-6]$/.test(sibling.tagName)) {
				if (sibling.tagName === 'TABLE') {
					const thead = sibling.querySelector('thead tr');
					if (thead) {
						const th = document.createElement('th');
						th.textContent = '#';
						th.className = 'text-muted-foreground';
						thead.insertBefore(th, thead.firstChild);
					}
					const rows = sibling.querySelectorAll('tbody tr');
					rows.forEach((row, i) => {
						const td = document.createElement('td');
						td.textContent = String(i + 1);
						td.className = 'text-muted-foreground';
						row.insertBefore(td, row.firstChild);
					});
					break;
				}
				sibling = sibling.nextElementSibling;
			}
		}
	}

	/** Colorize Status cells in Action Items / Requirements tables */
	const statusColorMap: Record<string, string> = {
		completed: 'background:rgb(209 250 229);color:rgb(6 95 70);border:1px solid rgb(167 243 208);',
		pending: 'background:rgb(254 252 232);color:rgb(161 98 7);border:1px solid rgb(254 240 138);',
		'in-progress': 'background:rgb(255 251 235);color:rgb(146 64 14);border:1px solid rgb(252 211 77);',
	};
	function colorizeStatusCells(el: HTMLElement) {
		const targetRe = /\b(action items|requirements)\b/i;
		const headings = el.querySelectorAll('h1, h2, h3, h4, h5, h6');
		for (const heading of headings) {
			if (!targetRe.test(heading.textContent ?? '')) continue;
			let sibling = heading.nextElementSibling;
			while (sibling && !/^H[1-6]$/.test(sibling.tagName)) {
				if (sibling.tagName === 'TABLE') {
					const headers = sibling.querySelectorAll('thead th');
					let statusIdx = -1;
					headers.forEach((th, i) => {
						if (th.textContent?.trim().toLowerCase() === 'status') statusIdx = i;
					});
					if (statusIdx === -1) break;
					const rows = sibling.querySelectorAll('tbody tr');
					for (const row of rows) {
						const cells = row.querySelectorAll('td');
						const cell = cells[statusIdx];
						if (!cell) continue;
						const val = cell.textContent?.trim().toLowerCase() ?? '';
						const style = statusColorMap[val];
						if (style) {
							cell.innerHTML = `<span style="${style}border-radius:0.375rem;padding:0.125rem 0.5rem;font-size:0.75rem;font-weight:500;white-space:nowrap;">${escapeHtml(cell.textContent?.trim() ?? '')}</span>`;
						}
					}
					break;
				}
				sibling = sibling.nextElementSibling;
			}
		}
	}

	/** Mark overdue due-date cells with red dashed underline (same style as unknown refs) */
	function markOverdueDates(el: HTMLElement) {
		const targetRe = /\b(action items|requirements)\b/i;
		const dateRe = /^\d{4}-\d{2}-\d{2}$/;
		const today = new Date();
		today.setHours(0, 0, 0, 0);

		const headings = el.querySelectorAll('h1, h2, h3, h4, h5, h6');
		for (const heading of headings) {
			if (!targetRe.test(heading.textContent ?? '')) continue;
			let sibling = heading.nextElementSibling;
			while (sibling && !/^H[1-6]$/.test(sibling.tagName)) {
				if (sibling.tagName === 'TABLE') {
					const headers = sibling.querySelectorAll('thead th');
					let dueDateIdx = -1;
					let statusIdx = -1;
					headers.forEach((th, i) => {
						const text = th.textContent?.trim().toLowerCase() ?? '';
						if (text === 'due date' || text === 'due') dueDateIdx = i;
						if (text === 'status') statusIdx = i;
					});
					if (dueDateIdx === -1) break;
					const rows = sibling.querySelectorAll('tbody tr');
					for (const row of rows) {
						const cells = row.querySelectorAll('td');
						const dateCell = cells[dueDateIdx];
						if (!dateCell) continue;
						const val = dateCell.textContent?.trim() ?? '';
						if (!dateRe.test(val)) continue;
						// Skip if status is completed/done
						if (statusIdx !== -1) {
							const statusVal = cells[statusIdx]?.textContent?.trim().toLowerCase() ?? '';
							if (statusVal === 'completed' || statusVal === 'done') continue;
						}
						const due = new Date(val + 'T00:00:00');
						if (due < today) {
							dateCell.innerHTML = `<span style="border-bottom:1.5px dashed var(--destructive);cursor:help;" title="Overdue — was due ${val}">${escapeHtml(val)}</span>`;
						}
					}
					break;
				}
				sibling = sibling.nextElementSibling;
			}
		}
	}

	/** Highlight code blocks with highlight.js (lazy-loaded) */
	async function highlightCodeBlocks(el: HTMLElement) {
		const blocks = el.querySelectorAll<HTMLElement>('pre code[class*="language-"]');
		if (blocks.length === 0) return;
		const hljs = (await import('highlight.js')).default;
		for (const block of blocks) {
			const lang = block.className.match(/language-(\w+)/)?.[1];
			if (lang === 'mermaid' || lang === 'd2' || lang === 'math') continue;
			hljs.highlightElement(block);
		}
	}

	/**
	 * Render KaTeX math following GitHub's rules:
	 * 1. ```math code fence → display math
	 * 2. `$...$` backtick-dollar inline code → inline math (allows special chars)
	 * 3. $$...$$ in prose → display math
	 * 4. $...$ in prose → inline math
	 */
	async function renderMath(el: HTMLElement) {
		const text = el.innerHTML;
		const hasDisplayMath = text.includes('$$');
		const hasInlineMath = /\$[^$\n]+\$/.test(text);
		const hasMathCodeBlock = el.querySelector('code.language-math') !== null;
		// Check for `$...$` backtick-dollar pattern: inline <code> with $ delimiters
		const hasBacktickDollar = el.querySelector('code') !== null && text.includes('$');
		if (!hasDisplayMath && !hasInlineMath && !hasMathCodeBlock && !hasBacktickDollar) return;

		const katex = (await import('katex')).default;

		// 1. ```math code blocks → display math
		const mathBlocks = el.querySelectorAll<HTMLElement>('pre code.language-math');
		for (const block of mathBlocks) {
			const pre = block.parentElement;
			if (!pre) continue;
			const src = block.textContent ?? '';
			const wrapper = document.createElement('div');
			wrapper.className = 'katex-display my-4';
			try {
				katex.render(src, wrapper, { displayMode: true, throwOnError: false });
			} catch {
				wrapper.textContent = src;
			}
			pre.replaceWith(wrapper);
		}

		// 2. `$...$` backtick-dollar → inline math (GitHub syntax for special chars)
		// Comrak renders `$x+1$` as <code>$x+1$</code> — detect inline <code> with $ delimiters
		const inlineCodes = el.querySelectorAll<HTMLElement>('code');
		for (const code of inlineCodes) {
			// Skip code blocks inside <pre> (those are fenced code blocks)
			if (code.parentElement?.tagName === 'PRE') continue;
			const raw = code.textContent ?? '';
			if (!raw.startsWith('$') || !raw.endsWith('$') || raw.length < 3) continue;
			const tex = raw.slice(1, -1);
			const span = document.createElement('span');
			span.className = 'katex-inline';
			try {
				katex.render(tex, span, { displayMode: false, throwOnError: false });
			} catch {
				continue; // Not valid math — leave the <code> as-is
			}
			code.replaceWith(span);
		}

		// 3+4. Walk text nodes for $$...$$ (display) and $...$ (inline)
		const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
		const textNodes: Text[] = [];
		let n: Node | null;
		while ((n = walker.nextNode())) {
			let skip = false;
			let p: Node | null = n.parentNode;
			while (p && p.nodeType === Node.ELEMENT_NODE) {
				const tag = (p as Element).tagName;
				if (tag === 'CODE' || tag === 'PRE' || tag === 'SCRIPT' || tag === 'STYLE') {
					skip = true;
					break;
				}
				p = p.parentNode;
			}
			if (!skip) textNodes.push(n as Text);
		}

		// $$...$$ = display math, $...$ = inline math
		// Inline: skip $digit (currency like $42K), limit to 100 chars, no leading/trailing space
		const mathRe = /\$\$([^$]+?)\$\$|\$(?!\d)([^$\n]{1,100}?)\$/g;

		for (const textNode of textNodes) {
			// Skip text inside hover cards (they contain prose with $ currency values)
			let inHoverCard = false;
			let p2: Node | null = textNode.parentNode;
			while (p2 && p2.nodeType === Node.ELEMENT_NODE) {
				if ((p2 as Element).classList?.contains('user-hovercard')) { inHoverCard = true; break; }
				p2 = p2.parentNode;
			}
			if (inHoverCard) continue;

			const txt = textNode.textContent ?? '';
			if (!txt.includes('$')) continue;
			const parts: (string | { tex: string; display: boolean })[] = [];
			let lastIdx = 0;
			let m;
			mathRe.lastIndex = 0;
			while ((m = mathRe.exec(txt)) !== null) {
				if (m.index > lastIdx) parts.push(txt.slice(lastIdx, m.index));
				if (m[1] !== undefined) {
					parts.push({ tex: m[1], display: true });
				} else {
					// Extra guard: skip if content looks like prose (contains ". " sentence breaks)
					if (m[2].includes('. ')) {
						parts.push(m[0]);
						lastIdx = m.index + m[0].length;
						continue;
					}
					parts.push({ tex: m[2], display: false });
				}
				lastIdx = m.index + m[0].length;
			}
			if (parts.length === 0) continue;
			if (lastIdx < txt.length) parts.push(txt.slice(lastIdx));

			const frag = document.createDocumentFragment();
			for (const part of parts) {
				if (typeof part === 'string') {
					frag.appendChild(document.createTextNode(part));
				} else {
					const span = document.createElement('span');
					span.className = part.display ? 'katex-display' : 'katex-inline';
					try {
						katex.render(part.tex, span, { displayMode: part.display, throwOnError: false });
					} catch {
						span.textContent = part.display ? `$$${part.tex}$$` : `$${part.tex}$`;
					}
					frag.appendChild(span);
				}
			}
			textNode.parentNode?.replaceChild(frag, textNode);
		}
	}

	/** Add anchor links to headings (GitHub-style, appended after text) */
	function addHeadingAnchors(el: HTMLElement) {
		const headings = el.querySelectorAll<HTMLElement>('h1, h2, h3, h4, h5, h6');
		for (const h of headings) {
			const text = h.textContent?.trim() ?? '';
			if (!text) continue;
			const slug = text.toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s+/g, '-').replace(/-+/g, '-');
			h.id = slug;
			const a = document.createElement('a');
			a.href = `#${slug}`;
			a.className = 'heading-anchor';
			a.setAttribute('aria-label', `Link to "${text}"`);
			a.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width=".75em" height=".75em" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>`;
			h.appendChild(a);
		}
	}

	/** Mount CodeCopyButton Svelte component into each <pre> block */
	function addCodeCopyButtons(el: HTMLElement) {
		const pres = el.querySelectorAll<HTMLElement>('pre');
		for (const pre of pres) {
			if (pre.querySelector('[data-copy-btn]')) continue;
			pre.style.position = 'relative';
			const wrapper = document.createElement('span');
			wrapper.setAttribute('data-copy-btn', '');
			pre.appendChild(wrapper);
			const code = pre.querySelector('code')?.textContent ?? pre.textContent ?? '';
			mount(CodeCopyButton, { target: wrapper, props: { code } });
		}
	}

	/**
	 * Auto-collapse developer-focused sections (e.g. "Local development").
	 * Wraps heading + all siblings until the next same-level heading in a <details>.
	 */
	function collapseDevSections(el: HTMLElement) {
		const devRe = /\b(local development|development setup|developer setup|dev environment|contributing|getting started)\b/i;
		const headings = el.querySelectorAll<HTMLElement>('h2');
		for (const h of headings) {
			const text = h.textContent?.trim() ?? '';
			if (!devRe.test(text)) continue;
			const level = parseInt(h.tagName[1]);

			// Collect all siblings after this heading until next same-or-higher-level heading
			const siblings: Node[] = [];
			let sibling = h.nextSibling;
			while (sibling) {
				if (sibling.nodeType === Node.ELEMENT_NODE) {
					const tag = (sibling as Element).tagName;
					if (/^H[1-6]$/.test(tag) && parseInt(tag[1]) <= level) break;
				}
				siblings.push(sibling);
				sibling = sibling.nextSibling;
			}

			const details = document.createElement('details');
			details.className = 'dev-section-collapsible';
			const summary = document.createElement('summary');
			// Move heading content into summary
			summary.innerHTML = `<strong>${h.innerHTML}</strong><span class="dev-section-hint">Technical setup for engineers — click to expand</span>`;
			details.appendChild(summary);
			const content = document.createElement('div');
			content.className = 'dev-section-content';
			for (const s of siblings) {
				content.appendChild(s);
			}
			details.appendChild(content);
			h.replaceWith(details);
		}
	}

	/** Combined action: all enrichments + syntax highlighting + math rendering */
	function enrichHtml(el: HTMLElement, content: string) {
		collapseDevSections(el);
		enrichContentRefs(el);
		enrichExistingDocLinks(el);
		enrichUserMentions(el);
		annotateConsequences(el);
		numberTableRows(el);
		colorizeStatusCells(el);
		markOverdueDates(el);
		addHeadingAnchors(el);
		addCodeCopyButtons(el);
		highlightCodeBlocks(el);
		renderMath(el);
		enrichFootnotes(el);

		// orgData loads async — re-run mention enrichment once it's available
		// so @handle mentions in body text get avatar links even on first page load
		let seenOrg = false;
		const unsub = orgData.subscribe((org) => {
			if (org && !seenOrg) {
				seenOrg = true;
				enrichContentRefs(el);
				enrichUserMentions(el);
			}
		});
		return { destroy: unsub };
	}

	function hrefToSelector(href: string): string {
		const id = href.startsWith('#') ? href.slice(1) : href;
		return `#${CSS.escape(id)}`;
	}

	function enrichFootnotes(el: HTMLElement) {
		const refs = el.querySelectorAll<HTMLAnchorElement>('sup.footnote-ref a');
		if (!refs.length) return;

		for (const ref of refs) {
			const href = ref.getAttribute('href');
			if (!href) continue;
			const fnLi = el.querySelector<HTMLElement>(hrefToSelector(href));
			if (!fnLi) continue;

			// Clone, strip backref, get clean text
			const clone = fnLi.cloneNode(true) as HTMLElement;
			clone.querySelectorAll('.footnote-backref').forEach((b) => b.remove());
			const text = (clone.textContent ?? '').trim();
			if (!text) continue;

			// Create tooltip span, append to the <sup>
			const tooltip = document.createElement('span');
			tooltip.className = 'footnote-tooltip';
			tooltip.textContent = text;
			const sup = ref.closest('sup');
			if (!sup) continue;
			sup.classList.add('footnote-has-tooltip');
			sup.appendChild(tooltip);

			// Smooth scroll on click
			ref.addEventListener('click', (e) => {
				e.preventDefault();
				fnLi.scrollIntoView({ behavior: 'smooth', block: 'center' });
				fnLi.classList.add('footnote-highlight');
				setTimeout(() => fnLi.classList.remove('footnote-highlight'), 2000);
			});
		}

		// Smooth scroll for back-references
		el.querySelectorAll<HTMLAnchorElement>('.footnote-backref').forEach((backref) => {
			backref.addEventListener('click', (e) => {
				e.preventDefault();
				const href = backref.getAttribute('href');
				if (!href) return;
				const target = el.querySelector<HTMLElement>(hrefToSelector(href));
				if (target) {
					target.scrollIntoView({ behavior: 'smooth', block: 'center' });
				}
			});
		});
	}

	function escapeHtml(s: string): string {
		return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
	}

	function d2ErrorHtml(msg: string, source: string): string {
		const retryId = `d2-retry-${Math.random().toString(36).slice(2, 9)}`;
		return `<div class="space-y-3">
			<div class="flex items-start gap-2 rounded-md border border-red-200 bg-red-50 dark:bg-red-950/30 dark:border-red-800 px-3 py-2">
				<span class="text-red-500 text-sm leading-none mt-0.5">⚠</span>
				<div class="min-w-0">
					<p class="text-sm font-medium text-red-700 dark:text-red-400">D2 diagram failed to render</p>
					<p class="text-xs text-red-600/80 dark:text-red-400/70 mt-0.5 break-all">${escapeHtml(msg)}</p>
				</div>
			</div>
			<details class="group">
				<summary class="cursor-pointer text-xs text-muted-foreground hover:text-foreground select-none">Show diagram source</summary>
				<pre class="mt-2 rounded-md bg-muted p-3 text-xs overflow-x-auto"><code>${escapeHtml(source)}</code></pre>
			</details>
		</div>`;
	}

	function renderD2(el: HTMLElement, source: string) {
		void renderD2Async(el, source);
	}

	async function renderD2Async(el: HTMLElement, source: string) {
		el.innerHTML = SPINNER_HTML;
		try {
			const { D2 } = await loadD2();
			const d2 = new D2();
			const result = await d2.compile(withDarkTheme(source));
			const svg = await d2.render(result.diagram, result.renderOptions);
			el.innerHTML = patchD2DarkCss(svg);
			// D2 outputs nested SVGs: outer viewBox="0 0 W H" clips inner .d2-svg
			// which uses negative offsets for label padding. Fix via DOM since SVG
			// string may start with XML prolog, making regex unreliable.
			const outer = el.querySelector('svg');
			if (outer) {
				if (!outer.getAttribute('width')) {
					outer.setAttribute('width', '100%');
					outer.removeAttribute('height');
					outer.style.height = 'auto';
				}
				outer.style.overflow = 'visible';
			}
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			console.error('D2 render failed:', err);
			el.innerHTML = d2ErrorHtml(msg, source);
		}
	}
</script>

<div class={className}>
	{#each segments as seg, i (i)}
		{#if seg.type === 'mermaid'}
			{#key $isDark}
				<div class="my-4 rounded-lg border border-border bg-muted p-4 overflow-x-auto">
					<div class="flex justify-center" use:renderMermaid={seg.content}></div>
				</div>
			{/key}
		{:else if seg.type === 'd2'}
			{#key $isDark}
				<div class="my-4 rounded-lg border border-border bg-muted p-4 overflow-x-auto">
					<div class="flex justify-center" use:renderD2={seg.content}></div>
				</div>
			{/key}
		{:else}
			<div use:enrichHtml={seg.content}>
				{@html seg.content}
			</div>
		{/if}
	{/each}
</div>
