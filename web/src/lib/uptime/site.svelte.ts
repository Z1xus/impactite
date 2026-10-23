import { untrack } from "svelte";
import { api, get } from "#lib/api.ts";
import type { Range } from "#lib/period.ts";
import { stream } from "./live.svelte.ts";
import type { Check, Health } from "./types.ts";

export type SiteIncident = {
	id: number;
	started_at: number;
	ended_at: number | null;
	cause: string;
	duration: number;
	visits_missed: number | null;
	start: string;
	end: string | null;
};

export type SiteUptime = {
	monitor: { id: number; state: Health; since: number } | null;
	incidents: SiteIncident[];
};

export function incidentsPath(domain: string, range?: Range | null) {
	const path = `/api/app/sites/${domain}/incidents`;
	return Array.isArray(range) ? `${path}?from=${range[0]}&to=${range[1]}` : path;
}

export function siteUptime(
	domain: () => string,
	range: () => Range | null,
	enabled: () => boolean,
	team: () => number | null
) {
	let data = $state.raw<SiteUptime | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function load(fresh = false) {
		const path = incidentsPath(domain(), range());
		try {
			const reply = await (fresh ? api : get)<SiteUptime>(path);
			if (path === incidentsPath(domain(), range())) data = reply;
		} catch {
			data = null;
		}
	}

	$effect(() => {
		if (!enabled()) {
			data = null;
			return;
		}
		void domain();
		void range();
		untrack(() => load());
		const refresh = setInterval(() => load(true), 60_000);
		return () => {
			clearInterval(refresh);
			clearTimeout(timer);
		};
	});

	stream(
		() => (enabled() ? team() : null),
		(event: Check) => {
			const known = data?.monitor;
			if (!data || !known || known.id !== event.id) return;
			if (known.state === event.state && known.since === event.since) return;
			data = { ...data, monitor: { ...known, state: event.state, since: event.since } };
			clearTimeout(timer);
			timer = setTimeout(() => load(true), 800);
		}
	);

	return {
		get value() {
			return data;
		}
	};
}
