<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import { display } from "#lib/display.svelte.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import { currentTeam, findSite, session, teamTimezone } from "#lib/session.svelte.ts";
	import Select from "#lib/ui/Select.svelte";
	import { warm } from "./warm.ts";

	let { domain, section = "overview" }: { domain: string; section?: "overview" | "heatmaps" } =
		$props();

	const combined = $derived(domain.startsWith("team:"));
	const team = $derived(currentTeam());
	const editable = $derived(["owner", "admin", "editor"].includes(findSite(domain)?.role ?? ""));
	const sites = $derived([
		...(section === "overview" && (team?.sites.length ?? 0) > 1 && display.all === "show"
			? [{ key: "all", label: "All sites" }]
			: []),
		...(session.me?.teams.flatMap((item) => item.sites) ?? []).map((site) => ({
			key: site.domain,
			label: site.domain,
			glyph: icon("event:hostname", site.domain)
		}))
	]);

	function open(key: string) {
		if (key === "all") return goto(resolve("/all"));
		if (section === "heatmaps") return goto(resolve("/[site]/heatmaps", { site: key }));
		return goto(resolve("/[site]", { site: key }));
	}
</script>

<Select
	value={combined ? "all" : domain}
	options={sites}
	data
	plain
	title
	footer={editable
		? {
				label: "Site settings",
				href: resolve("/[site]/settings/[[section]]", { site: domain }),
				icon: sliders
			}
		: undefined}
	warm={(key) => {
		if (section !== "overview") return;
		const site = findSite(key);
		if (team && key === "all") warm(`team:${team.id}`, teamTimezone(team));
		else if (site) warm(key, site.timezone);
	}}
	pick={open}
/>
