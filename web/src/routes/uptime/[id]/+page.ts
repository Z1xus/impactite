import { get } from "#lib/api.ts";
import { ready } from "#lib/ready.ts";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params }) => {
	await ready(get(`/api/app/uptime/${params.id}?range=24h`));
};
