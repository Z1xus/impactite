import { get } from "#lib/api.ts";
import { ready } from "#lib/ready.ts";
import { session } from "#lib/session.svelte.ts";
import type { PageLoad } from "./$types";

const READS: Record<string, string> = {
	sessions: "/api/app/me/sessions",
	keys: "/api/app/keys",
	passkeys: "/api/app/passkeys"
};

export const load: PageLoad = async ({ params: { section } }) => {
	const path = READS[section ?? ""];
	if (session.me && path) await ready(get(path));
};
