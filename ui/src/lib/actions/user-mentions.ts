import { get } from 'svelte/store';
import { orgData } from '$lib/stores/org';
import type { OrgData, UserDef, TeamDef, OrgDef } from '$lib/types';

export const AVATAR_COLORS = [
	['bg-blue-200', 'text-blue-800'],
	['bg-emerald-200', 'text-emerald-800'],
	['bg-amber-200', 'text-amber-800'],
	['bg-purple-200', 'text-purple-800'],
	['bg-pink-200', 'text-pink-800'],
	['bg-cyan-200', 'text-cyan-800'],
	['bg-orange-200', 'text-orange-800'],
	['bg-indigo-200', 'text-indigo-800']
];

export function colorIndex(handle: string): number {
	return handle.split('').reduce((a, c) => a + c.charCodeAt(0), 0) % AVATAR_COLORS.length;
}

export function initials(name: string): string {
	return name
		.split(/\s+/)
		.slice(0, 2)
		.map((w) => w[0]?.toUpperCase() ?? '')
		.join('');
}

export function teamNames(teamHandles: string[], org: OrgData): string[] {
	return teamHandles.map((t) => org.teams[t]?.name ?? t);
}

export function esc(s: string): string {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/** Build HTML for an unknown reference with dashed red underline + tooltip. */
export function buildUnknownRefHtml(text: string, tooltip: string): string {
	return `<span class="unknown-ref" style="border-bottom:1.5px dashed var(--destructive);cursor:help;" title="${esc(tooltip)}">${esc(text)}</span>`;
}

/** Build hover card inner HTML for a team. */
function teamHoverCardContent(handle: string, team: TeamDef, org: OrgData): string {
	const displayName = esc(team.name || handle);
	const leadName = team.lead ? esc(org.users[team.lead]?.name || team.lead) : '';
	const memberCount = team.members.length;
	return `<span class="font-medium text-sm">${displayName}</span>
		${leadName ? `<span class="text-xs text-muted-foreground">Lead: ${leadName}</span>` : ''}
		<span class="text-xs text-muted-foreground">${memberCount} member${memberCount !== 1 ? 's' : ''}</span>`;
}

/** Build HTML for a @team/ mention with link + hover card. */
export function buildTeamHtml(handle: string, team: TeamDef, org: OrgData): string {
	const content = teamHoverCardContent(handle, team, org);
	const hoverCard = `<span class="user-hovercard" style="display:none;">${content}</span>`;
	return `<a href="/org/teams/${esc(handle)}" class="group/mention relative inline-flex items-center gap-0.5"><span class="text-xs">👥</span><span class="underline decoration-dotted">@team/${esc(handle)}</span>${hoverCard}</a>`;
}

/** Build HTML for an @entity/ mention with link + hover card. */
export function buildOrgHtml(handle: string, orgDef: OrgDef): string {
	const displayName = esc(orgDef.name || handle);
	const content = `<span class="font-medium text-sm">${displayName}</span>`;
	const hoverCard = `<span class="user-hovercard" style="display:none;">${content}</span>`;
	return `<a href="/org/${esc(handle)}" class="group/mention relative inline-flex items-center gap-0.5"><span class="text-xs">🏢</span><span class="underline decoration-dotted">@entity/${esc(handle)}</span>${hoverCard}</a>`;
}

export function buildMentionHtml(handle: string, user: UserDef, org: OrgData): string {
	const [bg, fg] = AVATAR_COLORS[colorIndex(handle)];
	const ini = initials(user.name || handle);
	const displayName = esc(user.name || handle);
	const isExternal = user.kind === 'external';
	const teams = teamNames(user.teams, org);

	// Badge (link wrapping the initials circle)
	const badge = user.avatar_url
		? `<img src="${esc(user.avatar_url)}" alt="${displayName}" class="rounded-full object-cover w-6 h-6 shrink-0 align-middle" />`
		: `<span class="inline-flex items-center justify-center rounded-full font-medium w-6 h-6 text-[10px] ${bg} ${fg}">${ini}</span>`;

	// Hover card content
	const avatarLarge = user.avatar_url
		? `<img src="${esc(user.avatar_url)}" alt="${displayName}" class="rounded-full object-cover w-10 h-10 shrink-0" />`
		: `<span class="inline-flex items-center justify-center rounded-full font-medium w-10 h-10 text-sm ${bg} ${fg} shrink-0">${ini}</span>`;

	const titleLine = user.title ? `<span class="text-xs text-muted-foreground">${esc(user.title)}</span>` : '';
	const teamsLine =
		teams.length > 0
			? `<span class="text-xs text-muted-foreground">${esc(teams.join(', '))}</span>`
			: '';
	const externalBadge = isExternal
		? `<span class="inline-flex items-center rounded-full bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300 px-1.5 py-0.5 text-[10px] font-medium shrink-0">External</span>`
		: '';

	const hoverContent = `<span class="flex items-center gap-2">
			${avatarLarge}
			<span class="flex flex-col min-w-0">
				<span class="flex items-center gap-1.5">
					<span class="font-medium text-sm truncate">${displayName}</span>
					${externalBadge}
				</span>
				<span class="text-xs text-muted-foreground truncate">@${esc(handle)}</span>
			</span>
		</span>
		${titleLine}
		${teamsLine}`;

	const hoverCard = `<span class="user-hovercard" style="display:none;">${hoverContent}</span>`;

	return `<a href="/org/users/${esc(handle)}" class="group/mention relative inline-flex items-center h-6 overflow-hidden">${badge}${hoverCard}</a>`;
}

/** Shared floating hover card element, created once and reused. */
let floatingCard: HTMLDivElement | null = null;
let hideTimeout: ReturnType<typeof setTimeout> | null = null;

function getFloatingCard(): HTMLDivElement {
	if (floatingCard) return floatingCard;
	floatingCard = document.createElement('div');
	floatingCard.className =
		'fixed z-[9999] w-64 rounded-lg border border-border bg-popover text-popover-foreground shadow-md p-3 flex flex-col gap-1.5 pointer-events-none transition-opacity duration-150';
	floatingCard.style.opacity = '0';
	floatingCard.style.display = 'none';
	document.body.appendChild(floatingCard);
	return floatingCard;
}

export function showCard(trigger: HTMLElement) {
	if (hideTimeout) {
		clearTimeout(hideTimeout);
		hideTimeout = null;
	}
	const content = trigger.querySelector('.user-hovercard');
	if (!content) return;

	const card = getFloatingCard();
	card.innerHTML = content.innerHTML;
	card.style.display = 'flex';
	card.style.flexDirection = 'column';
	card.style.gap = '0.375rem';

	// Measure and position
	const rect = trigger.getBoundingClientRect();
	const cardRect = card.getBoundingClientRect();

	// Prefer above the trigger, centered horizontally
	let top = rect.top - cardRect.height - 8;
	let left = rect.left + rect.width / 2 - cardRect.width / 2;

	// If it would go above viewport, show below
	if (top < 4) {
		top = rect.bottom + 8;
	}

	// Clamp to viewport horizontally
	left = Math.max(4, Math.min(left, window.innerWidth - cardRect.width - 4));

	card.style.top = `${top}px`;
	card.style.left = `${left}px`;
	card.style.opacity = '1';
}

export function hideCard() {
	hideTimeout = setTimeout(() => {
		const card = getFloatingCard();
		card.style.opacity = '0';
		setTimeout(() => {
			card.style.display = 'none';
		}, 150);
	}, 100);
}

function attachHoverListeners(node: HTMLElement) {
	const triggers = node.querySelectorAll('.group\\/mention');
	for (const trigger of triggers) {
		trigger.addEventListener('mouseenter', () => showCard(trigger as HTMLElement));
		trigger.addEventListener('mouseleave', hideCard);
	}
}

function process(node: HTMLElement) {
	const org = get(orgData);
	if (!org?.users) return;
	const cells = node.querySelectorAll('td');
	for (const td of cells) {
		if (td.querySelector('.group\\/mention')) continue;
		const text = td.textContent?.trim();
		if (!text || !text.startsWith('@')) continue;
		const handle = text.slice(1);
		const user = org.users[handle];
		if (!user) continue;
		td.innerHTML = buildMentionHtml(handle, user, org);
	}
	attachHoverListeners(node);
}

/**
 * Svelte action that enriches @handle mentions in table cells with
 * avatar badges, links to user pages, and hover cards.
 * Apply to any container wrapping {@html} content.
 */
export function enrichUserMentions(node: HTMLElement) {
	process(node);
	return {
		update() {
			process(node);
		}
	};
}
