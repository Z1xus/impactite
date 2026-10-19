<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { icon } from "#lib/icons/resolve.ts";
	import { findSite, session } from "#lib/session.svelte.ts";
	import Collection from "#lib/settings/Collection.svelte";
	import SiteDanger from "#lib/settings/SiteDanger.svelte";
	import Data from "#lib/settings/Data.svelte";
	import Funnels from "#lib/settings/Funnels.svelte";
	import General from "#lib/settings/General.svelte";
	import Goals from "#lib/settings/Goals.svelte";
	import Guests from "#lib/settings/Guests.svelte";
	import Reports from "#lib/settings/Reports.svelte";
	import SearchConsole from "#lib/settings/SearchConsole.svelte";
	import Segments from "#lib/settings/Segments.svelte";
	import Sharing from "#lib/settings/Sharing.svelte";
	import Shields from "#lib/settings/Shields.svelte";
	import Tracking from "#lib/settings/Tracking.svelte";
	import bookmark from "pixelarticons/svg/bookmark.svg?raw";
	import code from "pixelarticons/svg/code.svg?raw";
	import download from "pixelarticons/svg/download.svg?raw";
	import eye from "pixelarticons/svg/eye.svg?raw";
	import filter from "pixelarticons/svg/filter.svg?raw";
	import mail from "pixelarticons/svg/mail.svg?raw";
	import search from "pixelarticons/svg/search.svg?raw";
	import share from "pixelarticons/svg/share.svg?raw";
	import shield from "pixelarticons/svg/shield.svg?raw";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import alert from "pixelarticons/svg/square-alert.svg?raw";
	import target from "pixelarticons/svg/target.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import Back from "#lib/ui/Back.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Missing from "#lib/ui/Missing.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Title from "#lib/ui/Title.svelte";

	const domain = $derived(page.params.site ?? "");
	const section = $derived(page.params.section ?? "general");
	const site = $derived(findSite(domain));
	const owner = $derived(!site || ["owner", "admin"].includes(site.role));
	const sections = $derived(
		[
			{ id: "general", label: "general", icon: sliders, group: "site" },
			{ id: "tracking", label: "tracking", icon: code, group: "site" },
			{ id: "collection", label: "collection", icon: eye, group: "site" },
			{ id: "shields", label: "shields", icon: shield, group: "site" },
			{ id: "goals", label: "goals", icon: target, group: "measure" },
			{ id: "funnels", label: "funnels", icon: filter, group: "measure" },
			{ id: "segments", label: "segments", icon: bookmark, group: "measure" },
			{ id: "imports", label: "data", icon: download, group: "measure" },
			{ id: "search-console", label: "search console", icon: search, group: "measure" },
			{ id: "sharing", label: "sharing", icon: share, group: "share" },
			{ id: "reports", label: "reports", icon: mail, group: "share" },
			...(owner
				? [
						{ id: "guests", label: "guests", icon: users, group: "share" },
						{ id: "danger", label: "danger", icon: alert, danger: true }
					]
				: [])
		].map((item) => ({
			...item,
			href: resolve("/[site]/settings/[[section]]", { site: domain, section: item.id })
		}))
	);
</script>

<Shell>
	{#if (site || !session.me) && sections.some((item) => item.id === section)}
		<Settings context={domain} {sections} current={section} loading={!site}>
			{#snippet title()}
				<Title>
					<Back href={resolve("/[site]", { site: domain })} title="Back to dashboard">
						<Glyph spec={icon("event:hostname", domain)} fallback={domain} />
						{domain}
					</Back>
				</Title>
			{/snippet}

			{#if site}
				{#key domain}
					{#if section === "general"}
						<General {domain} {site} {owner} />
					{:else if section === "tracking"}
						<Tracking {domain} />
					{:else if section === "collection"}
						<Collection {domain} {site} />
					{:else if section === "shields"}
						<Shields {domain} />
					{:else if section === "goals"}
						<Goals {domain} />
					{:else if section === "funnels"}
						<Funnels {domain} />
					{:else if section === "segments"}
						<Segments {domain} {owner} />
					{:else if section === "imports"}
						<Data {domain} />
					{:else if section === "search-console"}
						<SearchConsole {domain} />
					{:else if section === "sharing"}
						<Sharing {domain} {site} />
					{:else if section === "reports"}
						<Reports {domain} />
					{:else if section === "guests"}
						<Guests {domain} />
					{:else if section === "danger"}
						<SiteDanger {domain} />
					{/if}
				{/key}
			{/if}
		</Settings>
	{:else}
		<Missing />
	{/if}
</Shell>
