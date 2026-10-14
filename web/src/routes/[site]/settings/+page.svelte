<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import { goto } from "$app/navigation";
	import { refresh, session } from "#lib/session.svelte.ts";
	import Snippet from "#lib/Snippet.svelte";
	import Code from "#lib/ui/Code.svelte";
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
	type People = {
		guests: { id: number; name: string; email: string; role: string }[];
		invites: { id: number; email: string; role: string }[];
	};

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
	const owner = $derived(["owner", "admin"].includes(site?.role ?? ""));
	const home = $derived(
		session.me?.teams.find(
			(team) => team.id && team.sites.some((site) => site.domain === domain)
		)
	);
	const targets = $derived(
		(session.me?.teams ?? [])
			.filter((team) => team.id && ["owner", "admin"].includes(team.role))
			.map((team) => ({ key: team.id, label: team.personal ? "personal" : team.name }))
	);

	type Segment = { id: number; name: string; shared: boolean; mine: boolean };
	let segments = $state<Segment[] | null>(null);

	let people = $state<People | null>(null);
	let guestEmail = $state("");
	let guestRole = $state<"viewer" | "editor">("viewer");
	let guestLink = $state("");
	let error = $state("");

	$effect(() => {
		if (owner) api<People>(`/api/app/sites/${domain}/guests`).then((r) => (people = r));
	});

	async function inviteGuest() {
		const result = await api<{ link: string; sent: boolean }>(
			`/api/app/sites/${domain}/guests`,
			{
				email: guestEmail,
				role: guestRole
			}
		);
		guestLink = result.sent ? "" : result.link;
		guestEmail = "";
		people = await api<People>(`/api/app/sites/${domain}/guests`);
	}

	const removeGuest = (user: string | number) =>
		attempt(async () => {
			people = await api<People>(
				`/api/app/sites/${domain}/guests/${user}`,
				undefined,
				"DELETE"
			);
		});

	const revokeGuest = (invite: string | number) =>
		attempt(async () => {
			people = await api<People>(
				`/api/app/sites/${domain}/guests/invites/${invite}`,
				undefined,
				"DELETE"
			);
		});

	const moveTo = (team: number) =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}`, { team }, "PATCH");
			await refresh();
		});

	$effect(() => {
		if (!site) return;
		api<{ segments: Segment[] }>(`/api/app/sites/${domain}/segments`).then(
			(r) => (segments = r.segments)
		);
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

	const removeSegment = (id: string | number) =>
		attempt(async () => {
			segments = (
				await api<{ segments: Segment[] }>(
					`/api/app/sites/${domain}/segments/${id}`,
					undefined,
					"DELETE"
				)
			).segments;
		});

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
	{#if site}
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
					<Select
						value={site?.timezone ?? "UTC"}
						options={zones}
						data
						pick={setTimezone}
					/>
				</div>
			</Section>

			<Section label="goals">
				<List
					items={goals?.map((goal) => ({
						key: goal.id,
						title: goal.name,
						meta: [goal.page ? "page" : "event"]
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

			{#if segments?.length}
				<Section label="segments">
					<List
						items={segments.map((segment) => ({
							key: segment.id,
							title: segment.name,
							meta: [segment.shared ? "shared" : "personal"],
							removable: segment.mine || owner
						}))}
						remove={removeSegment}
					/>
				</Section>
			{/if}

			<Section label="imports">
				<List
					items={imports?.map((item) => ({
						key: item.id,
						title: `${item.first} to ${item.last}`,
						meta: [item.source]
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

			{#if owner && people}
				<Section label="guests">
					<List
						items={[
							...people.guests.map((guest) => ({
								key: `g${guest.id}`,
								title: guest.name,
								meta: [guest.email, guest.role]
							})),
							...people.invites.map((invite) => ({
								key: `i${invite.id}`,
								title: invite.email,
								meta: ["invited", invite.role]
							}))
						]}
						remove={(key) =>
							String(key).startsWith("g")
								? removeGuest(String(key).slice(1))
								: revokeGuest(String(key).slice(1))}
					/>
					<Form action="invite" submit={inviteGuest} inline>
						<Field
							label="email"
							type="email"
							placeholder="ada@example.com"
							bind:value={guestEmail}
							required
						/>
						<Toggle
							bind:value={guestRole}
							options={[
								{ key: "viewer", label: "viewer" },
								{ key: "editor", label: "editor" }
							]}
						/>
					</Form>
					{#if guestLink}
						<Code code={guestLink} />
						<Message>Mail is not set up, so share this link yourself.</Message>
					{/if}
				</Section>

				{#if home && targets.length > 1}
					<Section label="team">
						<div class="w-fit">
							<Select value={home.id} options={targets} pick={moveTo} />
						</div>
					</Section>
				{/if}
			{/if}

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
	{/if}
</Shell>
