import type { Filter } from "#lib/stats.ts";
import type { PanelSpec } from "./types.ts";

export const PAGES = new Set(["event:page", "visit:entry_page", "visit:exit_page"]);

export const PANELS: PanelSpec[] = [
	{
		id: "sources",
		tabs: [
			{ key: "visit:source", label: "sources" },
			{ key: "visit:channel", label: "channels" },
			{ key: "utm", label: "utm" }
		]
	},
	{
		id: "pages",
		tabs: [
			{ key: "event:page", label: "pages" },
			{ key: "visit:entry_page", label: "entry" },
			{ key: "visit:exit_page", label: "exit" },
			{ key: "flow", label: "flow" },
			{ key: "event:hostname", label: "hostnames" }
		]
	},
	{
		id: "locations",
		tabs: [
			{ key: "map", label: "map" },
			{ key: "visit:country", label: "countries" },
			{ key: "visit:region", label: "regions" },
			{ key: "visit:city", label: "cities" }
		]
	},
	{
		id: "devices",
		tabs: [
			{ key: "visit:browser", label: "browsers" },
			{ key: "visit:os", label: "systems" },
			{ key: "visit:device", label: "devices" },
			{ key: "visit:language", label: "languages" }
		]
	}
];

export const UTM = [
	{ key: "visit:utm_campaign", label: "campaign" },
	{ key: "visit:utm_source", label: "source" },
	{ key: "visit:utm_medium", label: "medium" },
	{ key: "visit:utm_content", label: "content" },
	{ key: "visit:utm_term", label: "term" }
];

// one value filtered on these shows the level below, like plausible
const DRILL: Record<string, { dim: string; label: string }> = {
	"visit:source": { dim: "visit:referrer", label: "referrers" },
	"visit:browser": { dim: "visit:browser_version", label: "versions" },
	"visit:os": { dim: "visit:os_version", label: "versions" }
};

export function drilled(filters: Filter[], tab: string) {
	const filter = filters.find((item) => item.dim === tab);
	return filter?.op === "is" && filter.values.length === 1 ? DRILL[tab] : undefined;
}
