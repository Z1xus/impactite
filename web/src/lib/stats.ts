import { api } from "./api";

export type Metric =
	"visitors" | "visits" | "pageviews" | "bounce_rate" | "visit_duration" | "views_per_visit";
export type Filter = { op: "is" | "is_not" | "contains"; dim: string; value: string };
export type Row = { dimensions: string[]; metrics: number[] };
export type Result = {
	results: Row[];
	meta?: { imports_included?: boolean; imports_skip_reason?: string };
};

export const METRICS: { key: Metric; label: string }[] = [
	{ key: "visitors", label: "visitors" },
	{ key: "visits", label: "visits" },
	{ key: "pageviews", label: "pageviews" },
	{ key: "bounce_rate", label: "bounce" },
	{ key: "visit_duration", label: "duration" },
	{ key: "views_per_visit", label: "views / visit" }
];

export const PERIODS = [
	{ key: "day", label: "today" },
	{ key: "7d", label: "7 days" },
	{ key: "30d", label: "30 days" },
	{ key: "90d", label: "90 days" },
	{ key: "12mo", label: "12 months" }
] as const;
export type Period = (typeof PERIODS)[number]["key"];

export function query(site: string, body: Record<string, unknown>, imports = true) {
	return api<Result>("/api/app/query", { site_id: site, include: { imports }, ...body });
}

export function today(timezone: string) {
	return new Intl.DateTimeFormat("en-CA", { timeZone: timezone }).format(new Date());
}

function shift(date: string, days: number) {
	const d = new Date(`${date}T00:00:00Z`);
	d.setUTCDate(d.getUTCDate() + days);
	return d.toISOString().slice(0, 10);
}

function span(from: string, to: string) {
	return Math.round((Date.parse(to) - Date.parse(from)) / 86_400_000) + 1;
}

export function range(period: Period, timezone: string) {
	const end = today(timezone);
	const days = { day: 1, "7d": 7, "30d": 30, "90d": 90, "12mo": 365 }[period];
	const from = period === "12mo" ? `${shift(end, -334).slice(0, 7)}-01` : shift(end, 1 - days);
	const length = span(from, end);
	return {
		current: [from, end] as [string, string],
		previous: [shift(from, -length), shift(from, -1)] as [string, string],
		grain: period === "day" ? "time:hour" : period === "12mo" ? "time:month" : "time:day"
	};
}

export function encodeFilter(filter: Filter) {
	return `${filter.op}~${filter.dim}~${filter.value}`;
}

export function decodeFilter(raw: string): Filter | null {
	const [op, dim, ...rest] = raw.split("~");
	if (!dim || !["is", "is_not", "contains"].includes(op)) return null;
	return { op: op as Filter["op"], dim, value: rest.join("~") };
}

export function toQueryFilters(filters: Filter[]) {
	return filters.map((filter) => [filter.op, filter.dim, [filter.value]]);
}

export function compact(value: number) {
	if (value < 10_000) return Math.round(value).toLocaleString("en-US");
	if (value < 1_000_000) return `${(value / 1000).toFixed(value < 100_000 ? 1 : 0)}k`;
	return `${(value / 1_000_000).toFixed(1)}m`;
}

export function format(metric: Metric, value: number) {
	switch (metric) {
		case "bounce_rate":
			return `${Math.round(value)}%`;
		case "visit_duration": {
			const seconds = Math.round(value);
			const minutes = Math.floor(seconds / 60);
			return minutes
				? `${minutes}m ${String(seconds % 60).padStart(2, "0")}s`
				: `${seconds}s`;
		}
		case "views_per_visit":
			return value.toFixed(2);
		default:
			return compact(value);
	}
}

export function delta(metric: Metric, current: number, previous: number) {
	if (!previous) return null;
	const change =
		metric === "bounce_rate" ? current - previous : ((current - previous) / previous) * 100;
	return {
		change: Math.round(change),
		good: metric === "bounce_rate" ? change <= 0 : change >= 0
	};
}

const countries = new Intl.DisplayNames(["en"], { type: "region" });

export function display(dim: string, value: string) {
	if (dim === "visit:country" && /^[A-Z]{2}$/.test(value)) return countries.of(value) ?? value;
	return value;
}
