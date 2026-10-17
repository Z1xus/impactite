import { save } from "#lib/download.ts";
import { METRICS, query, toQueryFilters } from "#lib/stats.ts";
import type { Context } from "./types.ts";

const quote = (value: unknown) => `"${String(value).replaceAll('"', '""')}"`;

export async function downloadCsv(ctx: Context) {
	const { domain, dates } = ctx;
	const result = await query(
		domain,
		{
			metrics: METRICS.map((item) => item.key),
			date_range: dates.current,
			dimensions: [dates.grain],
			filters: toQueryFilters(ctx.filters)
		},
		ctx.imports
	);
	const rows = [
		[
			dates.grain.slice(5),
			"visitors",
			"visits",
			"pageviews",
			"bounce_rate_percent",
			"visit_duration_seconds",
			"views_per_visit"
		],
		...result.results.map((row) => [...row.dimensions, ...row.metrics])
	];
	const range = Array.isArray(dates.current) ? dates.current.join("_") : dates.current;
	const csv = rows.map((row) => row.map(quote).join(",")).join("\r\n") + "\r\n";
	save(`${domain}_${range}.csv`, new Blob([csv], { type: "text/csv;charset=utf-8" }));
}
