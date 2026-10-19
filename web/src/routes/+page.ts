import { ready } from "#lib/ready.ts";
import { currentTeam } from "#lib/session.svelte.ts";
import { sparks } from "#lib/sparks.ts";
import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
	const team = currentTeam();
	if (team) await ready(sparks(team));
};
