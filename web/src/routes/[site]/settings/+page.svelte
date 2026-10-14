<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import { goto } from "$app/navigation";
	import { refresh, session } from "#lib/session.svelte.ts";
	import Snippet from "#lib/Snippet.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import Upload from "#lib/ui/Upload.svelte";

	type Goal = { id: number; page: boolean; value: string; name: string };
	type Import = { id: number; source: string; first: string; last: string };

	const zones = Intl.supportedValuesOf("timeZone").map((zone) => ({ key: zone, label: zone }));
	const domain = $derived(page.params.site ?? "");
	const site = $derived(
		session.me?.teams.flatMap((team) => team.sites).find((site) => site.domain === domain)
	);

	let goals = $state<Goal[] | null>(null);
	let imports = $state<Import[] | null>(null);
	let kind = $state<"event" | "page">("event");
	let value = $state("");
	let source = $state<"plausible" | "goatcounter">("plausible");
	let file = $state<File | null>(null);
	let imported = $state("");
	let confirm = $state("");
	const owner = $derived(
		["owner", "admin"].includes(
			session.me?.teams.find((team) => team.sites.some((site) => site.domain === domain))
				?.role ?? ""
		)
	);
	let error = $state("");

	$effect(() => {
		api<{ goals: Goal[] }>(`/api/app/sites/${domain}/goals`).then((r) => (goals = r.goals));
		api<{ imports: Import[] }>(`/api/app/sites/${domain}/imports`).then(
			(r) => (imports = r.imports)
		);
	});

	async function attempt(action: () => Promise<unknown>) {
		error = "";
		try {
			await action();
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		}
	}

	const setTimezone = (timezone: string) =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}`, { timezone }, "PATCH");
			await refresh();
		});

	async function addGoal() {
		goals = (
			await api<{ goals: Goal[] }>(`/api/app/sites/${domain}/goals`, {
				page: kind === "page",
				value
			})
		).goals;
		value = "";
	}

	const removeGoal = (id: string | number) =>
		attempt(async () => {
			goals = (
				await api<{ goals: Goal[] }>(
					`/api/app/sites/${domain}/goals/${id}`,
					undefined,
					"DELETE"
				)
			).goals;
		});

	async function runImport() {
		if (!file) throw new Error("Choose an export file.");
		imported = "";
		const result = await api<{ imported: string }>(
			`/api/app/sites/${domain}/imports?source=${source}`,
			file
		);
		imported = `Imported ${result.imported}.`;
		file = null;
		imports = (await api<{ imports: Import[] }>(`/api/app/sites/${domain}/imports`)).imports;
	}

	async function destroy() {
		if (confirm !== domain) throw new Error(`Type ${domain} to confirm.`);
		await api(`/api/app/sites/${domain}`, undefined, "DELETE");
		await refresh();
		await goto(resolve("/"));
	}

	const removeImport = (id: string | number) =>
		attempt(async () => {
			imports = (
				await api<{ imports: Import[] }>(
					`/api/app/sites/${domain}/imports/${id}`,
					undefined,
					"DELETE"
				)
			).imports;
		});
</script>

<Shell>
	<div class="flex flex-col gap-12">
		<a
			href={resolve("/[site]", { site: domain })}
			class="flex w-fit items-center gap-2.5 text-fg transition-colors duration-100 hover:text-cyan"
		>
			<Glyph spec={icon("event:hostname", domain)} fallback={domain} />
			{domain}
		</a>

		<Section label="tracking">
			<Snippet {domain} />
		</Section>

		<Section label="timezone">
			<div class="w-fit">
				<Select value={site?.timezone ?? "UTC"} options={zones} data pick={setTimezone} />
			</div>
		</Section>

		<Section label="goals">
			<List
				items={goals?.map((goal) => ({
					key: goal.id,
					title: goal.name,
					meta: goal.page ? "page" : "event"
				})) ?? null}
				remove={removeGoal}
			/>
			<Form action="add goal" submit={addGoal} inline>
				<Field
					label={kind === "page" ? "path" : "event name"}
					placeholder={kind === "page" ? "/pricing" : "Signup"}
					bind:value
					required
				/>
				<Toggle
					bind:value={kind}
					options={[
						{ key: "event", label: "event" },
						{ key: "page", label: "page" }
					]}
				/>
			</Form>
		</Section>

		<Section label="imports">
			<List
				items={imports?.map((item) => ({
					key: item.id,
					title: `${item.first} to ${item.last}`,
					meta: item.source
				})) ?? null}
				remove={removeImport}
			/>
			<Form action="import" submit={runImport} inline>
				<Upload bind:file accept={source === "plausible" ? ".zip" : ".csv,.gz"} />
				<Toggle
					bind:value={source}
					options={[
						{ key: "plausible", label: "plausible" },
						{ key: "goatcounter", label: "goatcounter" }
					]}
				/>
			</Form>
			<Message tone="cyan">{imported}</Message>
		</Section>

		{#if owner}
			<Section label="delete site">
				<Form action="delete" submit={destroy} inline>
					<Field
						label="type {domain} to delete it and all its data"
						bind:value={confirm}
					/>
				</Form>
			</Section>
		{/if}

		<Message tone="bad">{error}</Message>
	</div>
</Shell>
