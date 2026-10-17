import { api } from "#lib/api.ts";
import { refresh } from "#lib/session.svelte.ts";
import { attempt, toast } from "#lib/toast.svelte.ts";

export const sitePath = (domain: string) => `/api/app/sites/${domain}`;

export const saveSite = (domain: string, body: Record<string, unknown>) =>
	attempt(async () => {
		await api(sitePath(domain), body, "PATCH");
		await refresh();
		toast("Saved.");
	});
