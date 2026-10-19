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
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import SiteCard from "#lib/ui/SiteCard.svelte";
	import Title from "#lib/ui/Title.svelte";
	import { ZONES } from "#lib/zones.ts";

	const team = $derived(currentTeam());
	const editable = $derived(["owner", "admin", "editor"].includes(team?.role ?? ""));
	let series = $state<Record<string, number[]>>({});
	let domain = $state("");
	let failed = $state(false);
	let tries = $state(0);

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
		void tries;
		failed = false;
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
			})
				.then((result) => {
					if (!stale) series[target.key] = result.results.map((row) => row.metrics[0]);
				})
				.catch(() => {
					if (!stale) failed = true;
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

<svelte:head>
	<title>Sites · impactite</title>
</svelte:head>

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
		<div class="flex min-h-control items-center justify-between gap-4">
			<Title>
				{#if team}
					{team.personal ? "Personal" : team.name}
				{:else}
					<Skeleton class="h-6 w-40" />
				{/if}
			</Title>
			{#if editable && team?.sites.length}
				<Button onclick={() => (adding = true)}>Add site</Button>
			{/if}
		</div>
		{#if failed}
			<Empty compact retry={() => tries++} />
		{/if}
		{#if !team || team.sites.length}
			<ul class="grid grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),1fr))] gap-4">
				{#if !team}
					{#each [0, 1, 2] as placeholder (placeholder)}
						<li><SiteCard /></li>
					{/each}
				{:else}
					{#if team.sites.length > 1}
						<li>
							<SiteCard
								href={resolve("/all")}
								title="All sites"
								values={series.all}
							/>
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
				{/if}
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
