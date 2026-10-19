import { warm } from "#lib/dashboard/warm.ts";
import { ready } from "#lib/ready.ts";
import { findSite } from "#lib/session.svelte.ts";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params }) => {
	const site = findSite(params.site);
	if (site) await ready(warm(params.site, site.timezone));
	return { warmed: !!site };
};
