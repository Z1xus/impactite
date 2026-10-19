import { get } from "#lib/api.ts";
import { ready } from "#lib/ready.ts";
import { findSite } from "#lib/session.svelte.ts";
import type { PageLoad } from "./$types";

const READS: Record<string, string[]> = {
	goals: ["goals", "goals/suggestions"],
	funnels: ["funnels", "goals"],
	segments: ["segments"],
	data: ["imports"],
	"search-console": ["google"],
	sharing: ["shares"],
	reports: ["reports"],
	guests: ["guests"],
	shields: ["shields"]
};

export const load: PageLoad = async ({ params: { site, section } }) => {
	if (!findSite(site)) return;
	await ready(
		...(READS[section ?? ""] ?? []).map((path) => get(`/api/app/sites/${site}/${path}`))
	);
};
