<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api, type Site } from "#lib/api.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import { ZONES } from "#lib/zones.ts";
	import { saveSite, sitePath } from "./site.ts";

	let { domain, site, owner }: { domain: string; site: Site; owner: boolean } = $props();

	const RETENTION = [
		{ key: 0, label: "forever" },
		{ key: 30, label: "30 days" },
		{ key: 90, label: "90 days" },
		{ key: 180, label: "180 days" },
		{ key: 365, label: "1 year" },
		{ key: 730, label: "2 years" },
		{ key: 1095, label: "3 years" },
		{ key: 1825, label: "5 years" }
	];

	let name = $state(untrack(() => domain));
	const home = $derived(
		session.me?.teams.find((team) => team.id && team.sites.some((item) => item.id === site.id))
	);
	const targets = $derived(
		(session.me?.teams ?? [])
			.filter((team) => team.id && ["owner", "admin"].includes(team.role))
			.map((team) => ({ key: team.id, label: team.personal ? "personal" : team.name }))
	);

	async function rename() {
		await api(sitePath(domain), { domain: name }, "PATCH");
		await refresh();
		const next = session.me?.teams
			.flatMap((team) => team.sites)
			.find((item) => item.id === site.id)?.domain;
		if (next) await goto(resolve("/[site]/settings/[[section]]", { site: next }));
	}
</script>

<Section id="general" label="general">
	<Form action="Rename" done="Saved." submit={rename} inline>
		<Field label="domain" bind:value={name} required />
	</Form>
	<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
		<Select
			label="keep data"
			value={site.retention}
			options={RETENTION}
			pick={(retention) => saveSite(domain, { retention })}
		/>
		<Select
			label="timezone"
			value={site.timezone}
			options={ZONES}
			data
			pick={(timezone) => saveSite(domain, { timezone })}
		/>
		{#if owner && home && targets.length > 1}
			<Select
				label="team"
				value={home.id}
				options={targets}
				pick={(team) => saveSite(domain, { team })}
			/>
		{/if}
	</div>
</Section>
