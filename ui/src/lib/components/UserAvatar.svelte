<script lang="ts">
	import { orgData } from '$lib/stores/org';
	import { AVATAR_COLORS, colorIndex, initials as getInitials, showCard, hideCard } from '$lib/actions/user-mentions';

	interface Props {
		handle: string;
		name: string;
		avatarUrl?: string;
		size?: 'sm' | 'md';
	}

	let { handle, name, avatarUrl, size = 'sm' }: Props = $props();

	const sizeClass = $derived(size === 'md' ? 'w-12 h-12 text-lg' : 'w-7 h-7 text-xs');
	const ini = $derived(getInitials(name));
	const idx = $derived(colorIndex(handle));
	const colorClass = $derived(`${AVATAR_COLORS[idx][0]} ${AVATAR_COLORS[idx][1]}`);

	const user = $derived($orgData?.users[handle]);
	const teams = $derived(
		user?.teams
			?.map((t: string) => $orgData?.teams[t]?.name ?? t)
			.filter(Boolean) ?? []
	);
	const isExternal = $derived(user?.kind === 'external');
	const isDeparted = $derived(user?.status === 'departed');
	const departedClass = $derived(isDeparted ? 'opacity-50 grayscale' : '');
	const userHref = $derived(`/org/users/${handle}`);

	let triggerEl: HTMLElement | undefined = $state();

	function onEnter() {
		if (!triggerEl) return;
		showCard(triggerEl);
	}
</script>

<a
	href={userHref}
	bind:this={triggerEl}
	class="group/mention relative inline-flex items-center"
	onmouseenter={onEnter}
	onmouseleave={hideCard}
	onclick={(e) => e.stopPropagation()}
>
	{#if avatarUrl}
		<img
			src={avatarUrl}
			alt={name}
			class="rounded-full object-cover {sizeClass} {departedClass}"
		/>
	{:else}
		<span
			class="inline-flex items-center justify-center rounded-full font-medium {sizeClass} {colorClass} {departedClass}"
		>
			{ini}
		</span>
	{/if}
	<!-- Hidden hover card content, read by the floating card system -->
	<span class="user-hovercard" style="display:none;">
		<span class="flex items-center gap-2">
			{#if avatarUrl}
				<img src={avatarUrl} alt={name} class="rounded-full object-cover w-10 h-10 shrink-0 {departedClass}" />
			{:else}
				<span class="inline-flex items-center justify-center rounded-full font-medium w-10 h-10 text-sm {colorClass} shrink-0 {departedClass}">{ini}</span>
			{/if}
			<span class="flex flex-col min-w-0">
				<span class="flex items-center gap-1.5">
					<span class="font-medium text-sm truncate">{name}</span>
					{#if isDeparted}
						<span class="inline-flex items-center rounded-full bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300 px-1.5 py-0.5 text-[10px] font-medium shrink-0">Departed</span>
					{:else if isExternal}
						<span class="inline-flex items-center rounded-full bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300 px-1.5 py-0.5 text-[10px] font-medium shrink-0">External</span>
					{/if}
				</span>
				<span class="text-xs text-muted-foreground truncate">@{handle}</span>
			</span>
		</span>
		{#if user?.title}
			<span class="text-xs text-muted-foreground">{user.title}</span>
		{/if}
		{#if teams.length > 0}
			<span class="text-xs text-muted-foreground">{teams.join(', ')}</span>
		{/if}
	</span>
</a>
