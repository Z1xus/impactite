import { current, interval, parse, previous } from "#lib/period.ts";
import { METRICS, cached } from "#lib/stats.ts";

// starts the requests the dashboard makes first, with its default view
export function warm(domain: string, timezone: string) {
	const view = parse(new URLSearchParams(), timezone);
	const range = current(view);
	const before = previous(view, range);
	const metrics = METRICS.map((item) => item.key);
	const ask = (body: Record<string, unknown>) => cached(domain, body).catch(() => {});
	ask({ metrics: ["events"], date_range: "all" });
	ask({ metrics, date_range: range, filters: [] });
	if (before) ask({ metrics, date_range: before, filters: [] });
	ask({
		metrics: ["visitors"],
		date_range: range,
		dimensions: [`time:${interval(view, range)}`],
		filters: []
	});
}
