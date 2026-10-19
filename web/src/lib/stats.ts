import { number } from "./display.svelte.ts";
import { api } from "./api";

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
