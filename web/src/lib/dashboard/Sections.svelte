<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Nav from "#lib/ui/Nav.svelte";

	let { domain }: { domain: string } = $props();

	// the period carries over between the views that share it
	const search = $derived(page.url.search);
	const route = $derived(page.route.id ?? "");
</script>

<Nav
	label="site"
	items={[
		{
			label: "Overview",
			href: `${resolve("/[site]", { site: domain })}${route.endsWith("heatmaps") ? search : ""}`,
			current: route === "/[site]"
		},
		{
			label: "Heatmaps",
			href: `${resolve("/[site]/heatmaps", { site: domain })}${route === "/[site]" ? search : ""}`,
			current: route.endsWith("heatmaps")
		},
		{
			label: "Replays",
			href: resolve("/[site]/replays", { site: domain }),
			current: route.includes("replays")
		}
	]}
/>
