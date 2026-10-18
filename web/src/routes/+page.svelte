<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import { currentTeam, refresh, teamTimezone } from "#lib/session.svelte.ts";
	import { shift, today } from "#lib/period.ts";
	import { query } from "#lib/stats.ts";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import SiteCard from "#lib/ui/SiteCard.svelte";
	import Title from "#lib/ui/Title.svelte";
	import { ZONES } from "#lib/zones.ts";

	const team = $derived(currentTeam());
	const editable = $derived(["owner", "admin", "editor"].includes(team?.role ?? ""));
	let series = $state<Record<string, number[]>>({});
	let domain = $state("");

	const targets = $derived(
		JSON.stringify([
			...(team && team.sites.length > 1
				? [{ key: "all", domain: `team:${team.id}`, timezone: teamTimezone(team) }]
				: []),
			...(team?.sites ?? []).map((site) => ({ key: site.domain, ...site }))
		])
	);

	$effect(() => {
		let stale = false;
		series = {};
		for (const target of JSON.parse(targets) as {
			key: string;
			domain: string;
			timezone: string;
		}[])
			query(target.domain, {
				metrics: ["visitors"],
				date_range: [shift(today(target.timezone), -27), today(target.timezone)],
				dimensions: ["time:day"]
			}).then((result) => {
				if (!stale) series[target.key] = result.results.map((row) => row.metrics[0]);
			});
		return () => (stale = true);
	});

	let adding = $state(false);
	let timezone = $state(Intl.DateTimeFormat().resolvedOptions().timeZone);

	async function add() {
		const site = await api<{ domain: string }>("/api/app/sites", {
			team: team?.id,
			domain,
			timezone
		});
		domain = "";
		adding = false;
		await refresh();
		await goto(resolve("/[site]", { site: site.domain }));
	}
</script>

<Dialog title="add site" bind:open={adding}>
	<Form action="Add site" submit={add}>
		<Field label="domain" placeholder="example.com" bind:value={domain} required />
		<Select
			label="timezone"
			value={timezone}
			options={ZONES}
			data
			pick={(zone) => (timezone = zone)}
		/>
	</Form>
</Dialog>

<Shell>
	<div class="flex flex-col gap-6">
		<div class="flex min-h-9 items-center justify-between gap-4">
			<Title>{team?.personal ? "Personal" : (team?.name ?? "")}</Title>
			{#if editable && team?.sites.length}
				<Button onclick={() => (adding = true)}>Add site</Button>
			{/if}
		</div>
		{#if team && team.sites.length}
			<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
				{#if team.sites.length > 1}
					<li>
						<SiteCard href={resolve("/all")} title="All sites" values={series.all} />
					</li>
				{/if}
				{#each team.sites as site (site.id)}
					<li>
						<SiteCard
							href={resolve("/[site]", { site: site.domain })}
							title={site.domain}
							glyph={icon("event:hostname", site.domain)}
							values={series[site.domain]}
						/>
					</li>
				{/each}
			</ul>
		{:else}
			<Empty text="No sites yet.">
				{#if editable}
					<Button onclick={() => (adding = true)}>Add site</Button>
				{/if}
			</Empty>
		{/if}
	</div>
</Shell>
