<script lang="ts">
	import grid from "pixelarticons/svg/grid-2x2-2.svg?raw";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import Chart from "#lib/dither/Chart.svelte";
	import { icon } from "#lib/icons/resolve.ts";
	import { currentTeam, refresh } from "#lib/session.svelte.ts";
	import { shift, today } from "#lib/period.ts";
	import { compact, query } from "#lib/stats.ts";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Num from "#lib/ui/Num.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Title from "#lib/ui/Title.svelte";
	import { ZONES } from "#lib/zones.ts";

	const team = $derived(currentTeam());
	const editable = $derived(["owner", "admin", "editor"].includes(team?.role ?? ""));
	let series = $state<Record<string, number[]>>({});
	let domain = $state("");

	$effect(() => {
		if (team && team.sites.length > 1) {
			const timezone = [...team.sites].sort((a, b) => a.id - b.id)[0].timezone;
			query(`team:${team.id}`, {
				metrics: ["visitors"],
				date_range: [shift(today(timezone), -27), today(timezone)],
				dimensions: ["time:day"]
			}).then((result) => {
				series.all = result.results.map((row) => row.metrics[0]);
			});
		}
		for (const site of team?.sites ?? []) {
			query(site.domain, {
				metrics: ["visitors"],
				date_range: [shift(today(site.timezone), -27), today(site.timezone)],
				dimensions: ["time:day"]
			}).then((result) => {
				series[site.domain] = result.results.map((row) => row.metrics[0]);
			});
		}
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
	<Form action="add site" submit={add}>
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
			{#if editable}
				<Button onclick={() => (adding = true)}>add site</Button>
			{/if}
		</div>
		{#if team && team.sites.length}
			<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
				{#if team.sites.length > 1}
					{@const values = series.all}
					<li>
						<a
							href={resolve("/all")}
							class="flex flex-col gap-5 rounded-px border border-line bg-panel p-5 transition-colors duration-100 hover:border-edge"
						>
							<div class="flex items-center justify-between gap-4">
								<span class="flex items-center gap-2.5 text-fg">
									<Glyph spec={{ kind: "pixel", svg: grid }} />
									all sites
								</span>
								{#if values}
									<Num
										value={values.reduce((sum, value) => sum + value, 0)}
										format={compact}
										class="text-label text-dim"
									/>
								{/if}
							</div>
							<Chart
								values={values ?? []}
								axes={false}
								height={56}
								format={compact}
							/>
						</a>
					</li>
				{/if}
				{#each team.sites as site (site.id)}
					{@const values = series[site.domain]}
					<li>
						<a
							href={resolve("/[site]", { site: site.domain })}
							class="flex flex-col gap-5 rounded-px border border-line bg-panel p-5 transition-colors duration-100 hover:border-edge"
						>
							<div class="flex items-center justify-between gap-4">
								<span class="flex min-w-0 items-center gap-2.5">
									<Glyph
										spec={icon("event:hostname", site.domain)}
										fallback={site.domain}
									/>
									<span class="truncate text-fg">{site.domain}</span>
								</span>
								{#if values}
									<Num
										value={values.reduce((sum, value) => sum + value, 0)}
										format={compact}
										class="text-label text-dim"
									/>
								{/if}
							</div>
							<Chart
								values={values ?? []}
								axes={false}
								height={56}
								format={compact}
							/>
						</a>
					</li>
				{/each}
			</ul>
		{:else}
			<Message>No sites yet.</Message>
		{/if}
	</div>
</Shell>
