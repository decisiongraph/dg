import { get } from 'svelte/store';
import { allDocs, docTypes } from '$lib/stores/docs';
import { orgData } from '$lib/stores/org';
import { siteMeta } from '$lib/stores/site-meta';
import type { DocEntry, OrgData, TypeInfo } from '$lib/types';
import {
	buildMentionHtml,
	buildTeamHtml,
	buildOrgHtml,
	buildUnknownRefHtml,
	esc,
	showCard,
	hideCard
} from './user-mentions';

/** Tags whose text children should not be enriched. */
const SKIP_TAGS = new Set(['CODE', 'PRE', 'A', 'SCRIPT', 'STYLE']);

function shouldSkip(node: Node): boolean {
	let n: Node | null = node.parentNode;
	while (n && n.nodeType === Node.ELEMENT_NODE) {
		if (SKIP_TAGS.has((n as Element).tagName)) return true;
		n = n.parentNode;
	}
	return false;
}

/** Extract first heading + first paragraph from body HTML */
export function firstSection(html: string): { heading?: string; body: string } {
	const headingRe = /<h[1-6][^>]*>(.*?)<\/h[1-6]>/;
	const hMatch = html.match(headingRe);
	const strip = (s: string) => {
		const text = s.replace(/<[^>]+>/g, '').replace(/\s+/g, ' ').trim();
		// Decode HTML entities (e.g. &quot; → ") using DOM
		const el = document.createElement('textarea');
		el.innerHTML = text;
		return el.value;
	};

	if (hMatch) {
		const heading = strip(hMatch[1]);
		const after = html.slice(html.indexOf(hMatch[0]) + hMatch[0].length);
		const nextH = after.search(/<h[1-6]/);
		const sectionHtml = nextH >= 0 ? after.slice(0, nextH) : after;
		const pMatch = sectionHtml.match(/<p[^>]*>([\s\S]*?)<\/p>/);
		const body = pMatch ? strip(pMatch[1]) : strip(sectionHtml);
		return { heading, body };
	}

	const pMatch = html.match(/<p[^>]*>([\s\S]*?)<\/p>/);
	return { body: pMatch ? strip(pMatch[1]) : strip(html) };
}

/** Status → color mapping for doc hover cards. */
const STATUS_STYLES: Record<string, string> = {
	accepted: 'background:#d1fae5;color:#065f46;',
	proposed: 'background:#fef9c3;color:#854d0e;',
	deprecated: 'background:#fee2e2;color:#991b1b;',
	superseded: 'background:#e5e7eb;color:#374151;',
	exploring: 'background:#dbeafe;color:#1e40af;',
	pursuing: 'background:#fef3c7;color:#92400e;',
	completed: 'background:#d1fae5;color:#065f46;',
	resolved: 'background:#d1fae5;color:#065f46;',
	open: 'background:#fef9c3;color:#854d0e;',
	active: 'background:#dbeafe;color:#1e40af;',
	draft: 'background:#f3f4f6;color:#6b7280;',
	retired: 'background:#e5e7eb;color:#374151;'
};

function buildDocHoverCard(doc: DocEntry, folder: string): string {
	const statusStyle = STATUS_STYLES[doc.status?.toLowerCase()] ?? 'background:#f3f4f6;color:#6b7280;';
	const statusBadge = doc.status
		? `<span style="${statusStyle}border-radius:0.25rem;padding:0.125rem 0.375rem;font-size:0.625rem;font-weight:500;white-space:nowrap;">${esc(doc.status)}</span>`
		: '';

	const preview = doc.body_html ? firstSection(doc.body_html) : undefined;
	const bodySnippet = preview?.body
		? `<span class="text-xs text-muted-foreground leading-relaxed" style="display:block;">${preview.heading ? `<strong style="color:var(--foreground);opacity:0.8;">${esc(preview.heading)}:</strong> ` : ''}${esc(preview.body)}</span>`
		: '';

	const meta =
		doc.author || doc.date
			? `<span class="flex gap-2 text-[10px] text-muted-foreground pt-1" style="border-top:1px solid var(--border);">${doc.author ? `<span>@${esc(doc.author)}</span>` : ''}${doc.date ? `<span>${esc(doc.date)}</span>` : ''}</span>`
			: '';

	return `<a href="/${esc(folder)}/${esc(doc.id.toLowerCase())}" class="group/mention relative inline-flex items-center font-medium text-primary underline decoration-dotted underline-offset-2 hover:decoration-solid">
		${esc(doc.id)}
		<span class="user-hovercard" style="display:none;">
			<span class="flex items-center gap-2">
				<span class="font-mono text-xs text-muted-foreground">${esc(doc.id)}</span>
				${statusBadge}
			</span>
			<span class="text-sm font-medium leading-tight">${esc(doc.title)}</span>
			${bodySnippet}
			${meta}
		</span>
	</a>`;
}

function buildJiraLink(ticketId: string, baseUrl: string): string {
	const url = `${baseUrl}/${esc(ticketId)}`;
	return `<a href="${url}" target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-0.5 font-medium text-primary underline decoration-dotted underline-offset-2 hover:decoration-solid">${esc(ticketId)}<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="inline-block ml-0.5 opacity-50"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg></a>`;
}

function process(node: HTMLElement) {
	const docs = get(allDocs);
	const types = get(docTypes) as Record<string, TypeInfo>;
	const org = get(orgData);
	const meta = get(siteMeta);

	// Build Jira config map: prefix → url
	const jiraMap = new Map<string, string>();
	if (meta?.jira) {
		for (const j of meta.jira) {
			jiraMap.set(j.prefix.toUpperCase(), j.url);
		}
	}

	// Build lookup maps
	const docMap = new Map<string, DocEntry>();
	for (const d of docs) {
		docMap.set(d.id.toUpperCase(), d);
	}

	const knownIds = new Set(docMap.keys());

	// Build prefix regex from doc types + jira prefixes
	const prefixes = new Set<string>();
	for (const d of docs) {
		const prefix = d.id.split('-')[0];
		if (prefix) prefixes.add(prefix.toUpperCase());
	}
	for (const jp of jiraMap.keys()) {
		prefixes.add(jp);
	}
	if (prefixes.size === 0) return;

	const prefixPattern = Array.from(prefixes).join('|');
	const docIdRe = new RegExp(`\\b((?:${prefixPattern})-\\d+)\\b`, 'gi');
	const userRe = /@user\/([a-zA-Z0-9_-]+)/g;
	const teamRe = /@team\/([a-zA-Z0-9_-]+)/g;
	const entityRe = /@entity\/([a-zA-Z0-9_-]+)/g;
	const bareHandleRe = /(?<![a-zA-Z0-9.])@([a-zA-Z0-9_-]+)/g;

	// Walk text nodes
	const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
	const textNodes: Text[] = [];
	let current: Node | null;
	while ((current = walker.nextNode())) {
		textNodes.push(current as Text);
	}

	for (const textNode of textNodes) {
		if (shouldSkip(textNode)) continue;
		const text = textNode.textContent ?? '';

		// Find all matches with positions
		interface Match {
			start: number;
			end: number;
			html: string;
		}
		const matches: Match[] = [];

		// Doc IDs + Jira tickets
		for (const m of text.matchAll(docIdRe)) {
			const id = m[1].toUpperCase();
			const prefix = id.split('-')[0]?.toUpperCase();
			const jiraUrl = jiraMap.get(prefix ?? '');
			const doc = docMap.get(id);

			let html: string;
			if (doc) {
				const refType = prefix?.toLowerCase() ?? '';
				const folder = types[refType]?.folder ?? refType;
				html = buildDocHoverCard(doc, folder);
			} else if (jiraUrl) {
				html = buildJiraLink(id, jiraUrl);
			} else {
				html = buildUnknownRefHtml(m[0], 'Unknown document');
			}
			matches.push({ start: m.index!, end: m.index! + m[0].length, html });
		}

		// Explicit @user/, @team/, @entity/ prefixes
		if (org) {
			for (const m of text.matchAll(userRe)) {
				const handle = m[1];
				const user = org.users[handle];
				const html = user
					? buildMentionHtml(handle, user, org)
					: buildUnknownRefHtml(m[0], 'Unknown user');
				matches.push({ start: m.index!, end: m.index! + m[0].length, html });
			}

			for (const m of text.matchAll(teamRe)) {
				const handle = m[1];
				const team = org.teams[handle];
				const html = team
					? buildTeamHtml(handle, team, org)
					: buildUnknownRefHtml(m[0], 'Unknown team');
				matches.push({ start: m.index!, end: m.index! + m[0].length, html });
			}

			for (const m of text.matchAll(entityRe)) {
				const handle = m[1];
				const orgDef = org.orgs[handle];
				const html = orgDef
					? buildOrgHtml(handle, orgDef)
					: buildUnknownRefHtml(m[0], 'Unknown entity');
				matches.push({ start: m.index!, end: m.index! + m[0].length, html });
			}

			// Bare @handle — resolve against users, teams, entities; warn if ambiguous
			for (const m of text.matchAll(bareHandleRe)) {
				const start = m.index!;
				const end = start + m[0].length;
				// Skip if overlapping with an already-matched prefixed form
				if (matches.some((prev) => start >= prev.start && start < prev.end)) continue;
				const handle = m[1];
				const inUsers = handle in org.users;
				const inTeams = handle in org.teams;
				const inOrgs = handle in org.orgs;
				const count = +inUsers + +inTeams + +inOrgs;

				let html: string;
				if (count === 0) {
					html = buildUnknownRefHtml(m[0], 'Unknown user/team/entity');
				} else if (count > 1) {
					const kinds: string[] = [];
					if (inUsers) kinds.push('@user/' + handle);
					if (inTeams) kinds.push('@team/' + handle);
					if (inOrgs) kinds.push('@entity/' + handle);
					html = buildUnknownRefHtml(m[0], `Ambiguous — use ${kinds.join(' or ')}`);
				} else if (inUsers) {
					html = buildMentionHtml(handle, org.users[handle], org);
				} else if (inTeams) {
					html = buildTeamHtml(handle, org.teams[handle], org);
				} else {
					html = buildOrgHtml(handle, org.orgs[handle]);
				}
				matches.push({ start, end, html });
			}
		}

		if (matches.length === 0) continue;

		// Sort by position
		matches.sort((a, b) => a.start - b.start);

		// Deduplicate overlapping matches (keep first)
		const deduped: Match[] = [];
		let lastEnd = 0;
		for (const m of matches) {
			if (m.start >= lastEnd) {
				deduped.push(m);
				lastEnd = m.end;
			}
		}

		// Build fragment
		const frag = document.createDocumentFragment();
		let pos = 0;
		for (const m of deduped) {
			if (m.start > pos) {
				frag.appendChild(document.createTextNode(text.slice(pos, m.start)));
			}
			const span = document.createElement('span');
			span.innerHTML = m.html;
			// Unwrap the span to avoid nesting issues
			while (span.firstChild) {
				frag.appendChild(span.firstChild);
			}
			pos = m.end;
		}
		if (pos < text.length) {
			frag.appendChild(document.createTextNode(text.slice(pos)));
		}

		textNode.parentNode?.replaceChild(frag, textNode);
	}

	// Attach floating hover card listeners to all doc ref links
	const docRefLinks = node.querySelectorAll<HTMLElement>('.group\\/mention');
	for (const trigger of docRefLinks) {
		if ((trigger as any).__docRefHover) continue;
		(trigger as any).__docRefHover = true;
		trigger.addEventListener('mouseenter', () => showCard(trigger));
		trigger.addEventListener('mouseleave', () => hideCard());
	}
}

/**
 * Svelte action that enriches doc IDs and @mentions in {@html} content
 * with interactive links and hover cards.
 */
export function enrichContentRefs(node: HTMLElement) {
	process(node);
	return {
		update() {
			process(node);
		}
	};
}

/** Build just the hover popup span (no wrapping <a>) for injecting into existing links. */
function buildHoverPopup(doc: DocEntry): string {
	const statusStyle = STATUS_STYLES[doc.status?.toLowerCase()] ?? 'background:#f3f4f6;color:#6b7280;';
	const statusBadge = doc.status
		? `<span style="${statusStyle}border-radius:0.25rem;padding:0.125rem 0.375rem;font-size:0.625rem;font-weight:500;white-space:nowrap;">${esc(doc.status)}</span>`
		: '';

	const preview = doc.body_html ? firstSection(doc.body_html) : undefined;
	const bodySnippet = preview?.body
		? `<span class="text-xs text-muted-foreground leading-relaxed" style="display:block;">${preview.heading ? `<strong style="color:var(--foreground);opacity:0.8;">${esc(preview.heading)}:</strong> ` : ''}${esc(preview.body)}</span>`
		: '';

	const meta =
		doc.author || doc.date
			? `<span class="flex gap-2 text-[10px] text-muted-foreground pt-1" style="border-top:1px solid var(--border);">${doc.author ? `<span>@${esc(doc.author)}</span>` : ''}${doc.date ? `<span>${esc(doc.date)}</span>` : ''}</span>`
			: '';

	return `<span class="user-hovercard" style="display:none;">
		<span class="flex items-center gap-2">
			<span class="font-mono text-xs text-muted-foreground">${esc(doc.id)}</span>
			${statusBadge}
		</span>
		<span class="text-sm font-medium leading-tight">${esc(doc.title)}</span>
		${bodySnippet}
		${meta}
	</span>`;
}

/**
 * Enrich existing <a> links that point to doc IDs (e.g. href="opp-001.html")
 * with hover card popups. Used for pre-rendered HTML like the roadmap.
 */
export function enrichExistingDocLinks(node: HTMLElement) {
	const docs = get(allDocs);
	const types = get(docTypes) as Record<string, TypeInfo>;
	if (docs.length === 0) return;

	const docMap = new Map<string, DocEntry>();
	for (const d of docs) {
		docMap.set(d.id.toUpperCase(), d);
	}

	const links = node.querySelectorAll<HTMLAnchorElement>('a[href$=".html"]');
	for (const a of links) {
		if (a.querySelector('.user-hovercard')) continue; // already enriched
		const href = a.getAttribute('href') ?? '';
		const docId = href.replace(/\.html$/, '').toUpperCase();
		const doc = docMap.get(docId);
		if (!doc) continue;

		// Rewrite .html href to SPA route
		const prefix = doc.type?.toLowerCase() ?? docId.split('-')[0]?.toLowerCase();
		const folder = types[prefix]?.folder ?? prefix;
		a.setAttribute('href', `/${folder}/${doc.id.toLowerCase()}`);

		// Skip hover cards for gantt row labels (ID + title already visible)
		if (a.closest('.row-label')) continue;

		a.classList.add('group/mention', 'relative');
		const popup = document.createElement('span');
		popup.innerHTML = buildHoverPopup(doc);
		while (popup.firstChild) {
			a.appendChild(popup.firstChild);
		}

		a.addEventListener('mouseenter', () => showCard(a));
		a.addEventListener('mouseleave', () => hideCard());
	}
}
