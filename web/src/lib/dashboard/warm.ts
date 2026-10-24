import { get } from "#lib/api.ts";
import { current, interval, parse, previous } from "#lib/period.ts";
import { findSite } from "#lib/session.svelte.ts";
import {
	METRICS,
	cached,
	decodeFilter,
	hint,
	propKeys,
	toQueryFilters,
	type Filter
} from "#lib/stats.ts";
import { incidentsPath } from "#lib/uptime/site.svelte.ts";
import { PANELS } from "./panels.ts";

export function warm(
	domain: string,
	timezone: string,
	search = new URLSearchParams(),
	table = false
) {
	const view = parse(search, timezone);
	const metric = METRICS.find((item) => item.key === search.get("metric"))?.key ?? "visitors";
	const filters = toQueryFilters(
		search
			.getAll("f")
			.map(decodeFilter)
			.filter((filter): filter is Filter => filter !== null)
	);
	const imports = search.get("imports") !== "0";
	const range = current(view);
	const before = previous(view, range);
	const metrics = METRICS.map((item) => item.key);
	const grain = `time:${interval(view, range)}`;
	const keys = propKeys(domain, range);
	const asks: Promise<unknown>[] = [keys];
	const ask = (body: Record<string, unknown>) => {
		const reply = cached(domain, body, imports);
		asks.push(reply);
		return reply;
	};
	const top = (dimension: string, extra: string[] = []) =>
		ask({
			metrics: ["visitors", ...extra],
			date_range: range,
			dimensions: [dimension],
			filters,
			pagination: { limit: dimension === "visit:country" ? 300 : 9 }
		});
	const combined = domain.startsWith("team:");
	const site = (name: string) =>
		get<Record<string, unknown[]>>(`/api/app/sites/${domain}/${name}`);
	if (!combined) {
		if (findSite(domain)) asks.push(get(incidentsPath(domain, range)));
		if (["owner", "admin", "editor"].includes(findSite(domain)?.role ?? ""))
			asks.push(hint(domain));
		asks.push(
			site("titles"),
			site("segments"),
			site("funnels"),
			cached(domain, { metrics: ["events"], date_range: "all" })
		);
	}
	// the goals panel opens on its first tab that has data
	asks.push(
		Promise.all([combined ? { goals: [] } : site("goals"), keys]).then(([{ goals }, names]) => {
			if (goals.length) return top("event:goal", ["conversion_rate"]);
			if (names.length) return top(`event:props:${names[0]}`);
		})
	);
	ask({ metrics, date_range: range, filters });
	if (before) ask({ metrics, date_range: before, filters });
	ask({ metrics: table ? metrics : [metric], date_range: range, dimensions: [grain], filters });
	if (before && view.compare !== "off")
		ask({ metrics: [metric], date_range: before, dimensions: [grain], filters });
	for (const { tabs } of PANELS) top(tabs[0].key === "map" ? "visit:country" : tabs[0].key);
	return Promise.allSettled(asks);
}
