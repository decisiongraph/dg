import { base } from '$app/paths';

/**
 * Internal app paths are root-relative (`/org/users/x`) everywhere: in the
 * generated JSON, server-rendered doc HTML and UI code. The site may be hosted
 * under a subpath (e.g. `/demo/pied-piper/`); `base` is detected at runtime by
 * the HTML shell (see md-db `site/shell.rs`). These helpers are the single
 * place that maps between app paths and real URLs.
 */

/** Prefix a root-relative app path with the hosting base. Other URLs pass through. */
export function withBase(path: string): string {
	if (!path.startsWith('/') || path.startsWith('//')) return path;
	return base + path;
}

/** Inverse of `withBase` for `page.url.pathname`: strip the hosting base. */
export function stripBase(pathname: string): string {
	if (base && (pathname === base || pathname.startsWith(base + '/'))) {
		return pathname.slice(base.length) || '/';
	}
	return pathname;
}

/** Rebase root-relative `href`/`src` attributes inside server-rendered HTML. */
export function rebaseHtml(html: string): string {
	if (!base) return html;
	return html.replace(/\b(href|src)="\/(?!\/)/g, `$1="${base}/`);
}
