import { get } from "#lib/api.ts";
import { ready } from "#lib/ready.ts";
import { session } from "#lib/session.svelte.ts";
import type { PageLoad } from "./$types";

export const load: PageLoad = async () => {
	if (session.me?.admin) await ready(get("/api/app/admin"));
};
