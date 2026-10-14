<script lang="ts">
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import Chart from "#lib/dither/Chart.svelte";
	import { currentTeam, refresh } from "#lib/session.svelte.ts";
	import { compact, query, range } from "#lib/stats.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Num from "#lib/ui/Num.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	const team = $derived(currentTeam());
	const editable = $derived(team !== null && team.role !== "viewer");
	let series = $state<Record<string, number[]>>({});
	let domain = $state("");

	$effect(() => {
		for (const site of team?.sites ?? []) {
			query(site.domain, {
				metrics: ["visitors"],
				date_range: range("30d", site.timezone).current,
				dimensions: ["time:day"]
			}).then((result) => {
				series[site.domain] = result.results.map((row) => row.metrics[0]);
			});
		}
	});

	async function add() {
		await api("/api/app/sites", {
			team: team?.id,
			domain,
			timezone: Intl.DateTimeFormat().resolvedOptions().timeZone
		});
		domain = "";
		await refresh();
	}
</script>

<Shell>
	<div class="flex flex-col gap-10">
		{#if team && team.sites.length}
			<ul class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
				{#each team.sites as site (site.id)}
					{@const values = series[site.domain]}
					<li>
						<a
							href={resolve("/[site]", { site: site.domain })}
							class="flex flex-col gap-5 rounded-px border border-line bg-panel p-5 transition-colors duration-100 hover:border-edge"
						>
							<div class="flex items-baseline justify-between gap-4">
								<span class="truncate text-fg">{site.domain}</span>
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
		{/if}

		{#if editable}
			<div class="max-w-sm">
				<Form action="add site" submit={add} inline>
					<Field label="domain" bind:value={domain} placeholder="example.com" required />
				</Form>
			</div>
		{/if}
	</div>
</Shell>
