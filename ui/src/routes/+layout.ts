import { loadNav } from '$lib/stores/nav';
import { loadDocs } from '$lib/stores/docs';
import { loadSearchIndex } from '$lib/stores/search';
import { loadOrg } from '$lib/stores/org';
import { loadSiteMeta, loadReadme } from '$lib/stores/site-meta';
import { loadServices } from '$lib/stores/services';
import { loadSchema } from '$lib/stores/schema';

export const ssr = false;
export const prerender = false;

export const load = () => {
	// Fire all fetches without awaiting — let the UI render immediately
	// with loading skeletons. Each store sets its own loading flag.
	loadSiteMeta();
	loadReadme();
	loadNav();
	loadDocs();
	loadSearchIndex();
	loadOrg();
	loadServices();
	loadSchema();
	return {};
};
