import { warm } from "#lib/dashboard/warm.ts";
import { ready } from "#lib/ready.ts";
import { currentTeam, teamTimezone } from "#lib/session.svelte.ts";
import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
	const team = currentTeam();
	if (team?.sites.length) await ready(warm(`team:${team.id}`, teamTimezone(team)));
	return { warmed: !!team?.sites.length };
};
