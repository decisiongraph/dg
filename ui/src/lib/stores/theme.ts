import { writable } from 'svelte/store';

/** Whether the site is in dark mode. Shared between site-header toggle and diagram renderers. */
export const isDark = writable(false);
