import { number } from "./display.svelte.ts";
import { api, get } from "./api";
import type { Range } from "./period.ts";

export type Metric =
	"visitors" | "visits" | "pageviews" | "bounce_rate" | "visit_duration" | "views_per_visit";
export type Op = "is" | "is_not" | "contains" | "contains_not";
export type Filter = { op: Op; dim: string; values: string[] };

export const OPS: { key: Op; label: string }[] = [
	{ key: "is", label: "is" },
	{ key: "is_not", label: "is not" },
	{ key: "contains", label: "contains" },
	{ key: "contains_not", label: "not contains" }
];

export const DIMENSIONS: { key: string; label: string }[] = [
	{ key: "event:page", label: "page" },
	{ key: "visit:entry_page", label: "entry page" },
	{ key: "visit:exit_page", label: "exit page" },
	{ key: "event:hostname", label: "hostname" },
	{ key: "visit:source", label: "source" },
	{ key: "visit:referrer", label: "referrer" },
	{ key: "visit:channel", label: "channel" },
	{ key: "visit:utm_medium", label: "utm medium" },
	{ key: "visit:utm_source", label: "utm source" },
	{ key: "visit:utm_campaign", label: "utm campaign" },
	{ key: "visit:utm_content", label: "utm content" },
	{ key: "visit:utm_term", label: "utm term" },
	{ key: "visit:country", label: "country" },
	{ key: "visit:region", label: "region" },
	{ key: "visit:city", label: "city" },
	{ key: "visit:device", label: "device" },
	{ key: "visit:browser", label: "browser" },
	{ key: "visit:browser_version", label: "browser version" },
	{ key: "visit:os", label: "system" },
	{ key: "visit:os_version", label: "system version" },
	{ key: "visit:language", label: "language" },
	{ key: "event:goal", label: "goal" }
];

export function dimLabel(dim: string) {
	if (dim.startsWith("event:props:")) return dim.slice(12);
	return DIMENSIONS.find((item) => item.key === dim)?.label ?? dim;
}
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

export function query(site: string, body: Record<string, unknown>, imports = true) {
	return api<Result>("/api/app/query", { site_id: site, include: { imports }, ...body });
}

export type Driver = { dimension: string; value: string; delta: number };
export type Insights = {
	change: { percent: number; drivers: Driver[] } | null;
	spikes: { day: string; excess: number; drivers: Driver[] }[];
};

export const insights = (site: string, body: Record<string, unknown>) =>
	api<Insights>("/api/app/insights", { site_id: site, ...body });

export type Rating = "good" | "ni" | "poor";
export type Vital = {
	key: string;
	samples: number;
	p75?: number;
	rating?: Rating;
	shares?: number[];
	page?: string;
};

export const VITALS = [
	{ key: "lcp", label: "LCP", title: "Largest contentful paint" },
	{ key: "inp", label: "INP", title: "Interaction to next paint" },
	{ key: "cls", label: "CLS", title: "Cumulative layout shift" },
	{ key: "fcp", label: "FCP", title: "First contentful paint" },
	{ key: "ttfb", label: "TTFB", title: "Time to first byte" }
];

export const vitals = (site: string, body: Record<string, unknown>) =>
	api<{ vitals: Vital[] }>("/api/app/vitals", { site_id: site, ...body });

export const vitalPages = (site: string, body: Record<string, unknown>) =>
	api<{ pages: Vital[] }>("/api/app/vitals/pages", { site_id: site, ...body });

export const vitalTrend = (site: string, body: Record<string, unknown>) =>
	api<{ trend: { time: string; p75: number; samples: number }[]; good: number; poor: number }>(
		"/api/app/vitals/trend",
		{ site_id: site, ...body }
	);

export function speed(key: string, value: number) {
	if (key === "cls") return number(value, 2);
	return value >= 1000 ? `${number(value / 1000, 1)} s` : `${Math.round(value)} ms`;
}

export const propKeys = (site: string, range: Range) =>
	get<string[]>("/api/app/props", {
		site_id: site,
		date_range: range === "30m" ? "day" : range
	});

const recent = new Map<string, { at: number; reply: Promise<Result> }>();
const FRESH = 60_000;

// repeat queries share one request for a minute, so a hover prefetch feeds the page it opens
export function cached(site: string, body: Record<string, unknown>, imports = true) {
	const key = JSON.stringify([site, body, imports]);
	const hit = recent.get(key);
	if (hit && body.date_range !== "30m" && Date.now() - hit.at < FRESH) return hit.reply;
	const reply = query(site, body, imports);
	recent.set(key, { at: Date.now(), reply });
	reply.catch(() => recent.delete(key));
	return reply;
}

export type NewEvent = { name: string; count: number; revenue: boolean };

const hints = new Map<string, { at: number; reply: Promise<NewEvent[]> }>();

const events = (site: string) =>
	api<{ events: NewEvent[] }>(`/api/app/sites/${site}/goals/suggestions`).then(
		(result) => result.events
	);

export function hint(site: string) {
	const hit = hints.get(site);
	if (hit && Date.now() - hit.at < FRESH) return hit.reply;
	const reply = events(site);
	hints.set(site, { at: Date.now(), reply });
	reply.catch(() => hints.delete(site));
	return reply;
}

// a hovered site's suggestions are used once, so goals changed in between never show stale
export function newEvents(site: string) {
	const hit = hints.get(site);
	hints.delete(site);
	return hit && Date.now() - hit.at < FRESH ? hit.reply : events(site);
}

const escape = (value: string) => value.replaceAll("%", "%25").replaceAll("~", "%7E");
const unescape = (value: string) => value.replaceAll("%7E", "~").replaceAll("%25", "%");

export function encodeFilter(filter: Filter) {
	return [filter.op, filter.dim, ...filter.values.map(escape)].join("~");
}

export function decodeFilter(raw: string): Filter | null {
	const [op, dim, ...values] = raw.split("~");
	if (!dim || !values.length || !OPS.some((item) => item.key === op)) return null;
	return { op: op as Op, dim, values: values.map(unescape) };
}

export function toQueryFilters(filters: Filter[]) {
	return filters.map((filter) => [filter.op, filter.dim, filter.values]);
}

export function compact(value: number) {
	if (value < 10_000) return number(Math.round(value));
	// thresholds where rounding would print 100.0k or 1,000k
	if (value < 999_500) return `${number(value / 1000, value < 99_950 ? 1 : 0)}k`;
	return `${number(value / 1_000_000, 1)}m`;
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
			return number(value, 2);
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
const languages = new Intl.DisplayNames(["en"], { type: "language" });

export function display(dim: string, value: string) {
	if (dim === "visit:referrer" && value === "(none)") return "Direct / None";
	if (dim === "visit:country" && /^[A-Z]{2}$/.test(value)) return countries.of(value) ?? value;
	if (dim === "visit:language" && /^[a-z]{2,3}$/.test(value)) return languages.of(value) ?? value;
	return value;
}
