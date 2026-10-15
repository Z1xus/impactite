<script lang="ts">
	import { untrack } from "svelte";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { attempt } from "#lib/toast.svelte.ts";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import { goto } from "$app/navigation";
	import { refresh, session } from "#lib/session.svelte.ts";
	import Snippet from "#lib/Snippet.svelte";
	import close from "pixelarticons/svg/close.svg?raw";
	import Button from "#lib/ui/Button.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
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
	import Settings from "#lib/ui/Settings.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Upload from "#lib/ui/Upload.svelte";
	import { ZONES } from "#lib/zones.ts";

	type Goal = {
		id: number;
		page: boolean;
		value: string;
		name: string;
		scroll: number;
		currency: string;
	};
	type Import = { id: number; source: string; first: string; last: string };
	type People = {
		guests: { id: number; name: string; email: string; role: string }[];
		invites: { id: number; email: string; role: string }[];
	};

	const domain = $derived(page.params.site ?? "");
	const site = $derived(
		session.me?.teams.flatMap((team) => team.sites).find((site) => site.domain === domain)
	);

	let goals = $state<Goal[] | null>(null);
	let imports = $state<Import[] | null>(null);
	let kind = $state<"event" | "page" | "revenue">("event");
	let value = $state("");
	let goalName = $state("");
	let scroll = $state("");
	let currency = $state("EUR");
	let adding = $state(false);
	let renaming = $state<Goal | null>(null);
	type Funnel = { id: number; name: string; steps: number[] };
	type Kind = "ip" | "country" | "page" | "hostname" | "prop";
	type Shield = { id: number; kind: Kind; value: string };
	const KINDS: { key: Kind; label: string; meta: string; example: string }[] = [
		{ key: "ip", label: "ip", meta: "blocked ip", example: "203.0.113.0/24" },
		{ key: "country", label: "country", meta: "blocked country", example: "NL" },
		{ key: "page", label: "page", meta: "blocked page", example: "/admin/*" },
		{ key: "hostname", label: "hostname", meta: "allowed hostname", example: "*.example.com" },
		{ key: "prop", label: "prop", meta: "allowed prop", example: "plan" }
	];
	const IGNORE = 'localStorage.plausible_ignore = "true"';
	type Link = { id: number; slug: string; name: string; locked: boolean };
	type ReportKind = "daily" | "weekly" | "biweekly" | "monthly" | "spike" | "drop";
	type Report = { id: number; kind: ReportKind; email: string; threshold: number };
	const REPORTS: { key: ReportKind; label: string; meta: (threshold: number) => string }[] = [
		{ key: "daily", label: "daily", meta: () => "daily report" },
		{ key: "weekly", label: "weekly", meta: () => "weekly report" },
		{ key: "biweekly", label: "biweekly", meta: () => "report every two weeks" },
		{ key: "monthly", label: "monthly", meta: () => "monthly report" },
		{ key: "spike", label: "spike", meta: (n) => `alert at ${n} current visitors` },
		{ key: "drop", label: "drop", meta: (n) => `alert under ${n} visitors a day` }
	];
	let reports = $state<Report[] | null>(null);
	type Google = {
		configured: boolean;
		redirect: string;
		connected: boolean;
		property: string;
		properties: string[];
	};
	let google = $state<Google | null>(null);
	const googleFailed = new URLSearchParams(location.search).get("google") === "failed";
	let reporting = $state(false);
	let reportKind = $state<ReportKind>("weekly");
	let reportEmail = $state(session.me?.email ?? "");
	let reportThreshold = $state("");
	let links = $state<Link[] | null>(null);
	let linking = $state(false);
	let linkName = $state("");
	let linkPassword = $state("");
	const publicLink = $derived(links?.find((link) => link.slug === domain));
	const embedLink = $derived(publicLink ?? links?.find((link) => !link.locked) ?? links?.[0]);
	const url = (slug: string) => `${location.origin}/share/${slug}`;
	let shields = $state<Shield[] | null>(null);
	let shielding = $state(false);
	let shieldKind = $state<Kind>("ip");
	let shieldValue = $state("");
	let funnels = $state<Funnel[] | null>(null);
	let building = $state(false);
	let funnelName = $state("");
	let steps = $state<number[]>([]);
	let newName = $state("");
	let source = $state<"impactite" | "plausible" | "goatcounter">("impactite");
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
		api<{ funnels: Funnel[] }>(`/api/app/sites/${domain}/funnels`).then(
			(r) => (funnels = r.funnels)
		);
		api<{ shields: Shield[] }>(`/api/app/sites/${domain}/shields`).then(
			(r) => (shields = r.shields)
		);
		api<{ shares: Link[] }>(`/api/app/sites/${domain}/shares`).then((r) => (links = r.shares));
		api<{ reports: Report[] }>(`/api/app/sites/${domain}/reports`).then(
			(r) => (reports = r.reports)
		);
		api<Google>(`/api/app/sites/${domain}/google`)
			.then((r) => (google = r))
			.catch(() => (google = null));
		api<{ segments: Segment[] }>(`/api/app/sites/${domain}/segments`).then(
			(r) => (segments = r.segments)
		);
		api<{ goals: Goal[] }>(`/api/app/sites/${domain}/goals`).then((r) => (goals = r.goals));
		api<{ imports: Import[] }>(`/api/app/sites/${domain}/imports`).then(
			(r) => (imports = r.imports)
		);
	});

	const setTimezone = (timezone: string) =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}`, { timezone }, "PATCH");
			await refresh();
		});

	async function addGoal() {
		goals = (
			await api<{ goals: Goal[] }>(`/api/app/sites/${domain}/goals`, {
				page: kind === "page",
				value,
				name: goalName,
				scroll: kind === "page" ? Number(scroll) || 0 : 0,
				currency: kind === "revenue" ? currency : ""
			})
		).goals;
		value = goalName = scroll = "";
		adding = false;
	}

	async function addShield() {
		shields = (
			await api<{ shields: Shield[] }>(`/api/app/sites/${domain}/shields`, {
				kind: shieldKind,
				value: shieldValue
			})
		).shields;
		shieldValue = "";
		shielding = false;
	}

	const removeShield = (id: string | number) =>
		attempt(async () => {
			shields = (
				await api<{ shields: Shield[] }>(
					`/api/app/sites/${domain}/shields/${id}`,
					undefined,
					"DELETE"
				)
			).shields;
		});

	const setPublic = (on: boolean) =>
		attempt(async () => {
			links = (
				on
					? await api<{ shares: Link[] }>(`/api/app/sites/${domain}/shares`, {
							name: "public",
							public: true
						})
					: await api<{ shares: Link[] }>(
							`/api/app/sites/${domain}/shares/${publicLink?.id}`,
							undefined,
							"DELETE"
						)
			).shares;
		});

	async function addLink() {
		links = (
			await api<{ shares: Link[] }>(`/api/app/sites/${domain}/shares`, {
				name: linkName,
				password: linkPassword
			})
		).shares;
		linkName = linkPassword = "";
		linking = false;
	}

	const removeLink = (id: string | number) =>
		attempt(async () => {
			links = (
				await api<{ shares: Link[] }>(
					`/api/app/sites/${domain}/shares/${id}`,
					undefined,
					"DELETE"
				)
			).shares;
		});

	const chooseProperty = (property: string) =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}/google`, { property }, "PATCH");
			if (google) google.property = property;
		});

	const disconnectGoogle = () =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}/google`, undefined, "DELETE");
			google = await api<Google>(`/api/app/sites/${domain}/google`);
		});

	async function addReport() {
		reports = (
			await api<{ reports: Report[] }>(`/api/app/sites/${domain}/reports`, {
				kind: reportKind,
				email: reportEmail,
				threshold: Number(reportThreshold) || 0
			})
		).reports;
		reporting = false;
	}

	const removeReport = (id: string | number) =>
		attempt(async () => {
			reports = (
				await api<{ reports: Report[] }>(
					`/api/app/sites/${domain}/reports/${id}`,
					undefined,
					"DELETE"
				)
			).reports;
		});

	async function myIp() {
		shieldValue = (await api<{ ip: string }>("/api/app/ip")).ip;
	}

	function startFunnel() {
		funnelName = "";
		steps = (goals ?? []).slice(0, 2).map((goal) => goal.id);
		building = true;
	}

	async function addFunnel() {
		funnels = (
			await api<{ funnels: Funnel[] }>(`/api/app/sites/${domain}/funnels`, {
				name: funnelName,
				steps
			})
		).funnels;
		building = false;
	}

	const removeFunnel = (id: string | number) =>
		attempt(async () => {
			funnels = (
				await api<{ funnels: Funnel[] }>(
					`/api/app/sites/${domain}/funnels/${id}`,
					undefined,
					"DELETE"
				)
			).funnels;
		});

	async function renameGoal() {
		if (!renaming) return;
		goals = (
			await api<{ goals: Goal[] }>(
				`/api/app/sites/${domain}/goals/${renaming.id}`,
				{ name: newName },
				"PATCH"
			)
		).goals;
		renaming = null;
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
	let domainName = $state(untrack(() => page.params.site ?? ""));
	let resetConfirm = $state("");
	let purgePath = $state("");

	async function renameSite() {
		const id = site?.id;
		await api(`/api/app/sites/${domain}`, { domain: domainName }, "PATCH");
		await refresh();
		const next = session.me?.teams
			.flatMap((team) => team.sites)
			.find((item) => item.id === id)?.domain;
		if (next) await goto(resolve("/[site]/settings", { site: next }));
	}

	// split so the svelte parser does not end this block
	const scriptEnd = "</" + "script>";
	const counterSnippet = `<script>window.goatcounter = { no_onload: true }${scriptEnd}\n<script async src="${location.origin}/count.js" onload="goatcounter.visit_count({ append: 'body' })">${scriptEnd}`;

	const COLLECTED = [
		{ key: "referrer", label: "referrers" },
		{ key: "agent", label: "browsers and systems" },
		{ key: "device", label: "devices" },
		{ key: "location", label: "countries" },
		{ key: "region", label: "regions and cities" },
		{ key: "language", label: "languages" },
		{ key: "session", label: "sessions" }
	];
	const setCollected = (collected: string[]) =>
		attempt(async () => {
			const skip = COLLECTED.map((item) => item.key).filter(
				(key) => !collected.includes(key)
			);
			await api(`/api/app/sites/${domain}`, { skip }, "PATCH");
			await refresh();
		});

	const setCounter = (counter: boolean) =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}`, { counter }, "PATCH");
			await refresh();
		});

	const setRetention = (retention: number) =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}`, { retention }, "PATCH");
			await refresh();
		});

	async function purgePage() {
		if (!purgePath.startsWith("/")) throw new Error("Use a path like /old-page.");
		await api(`/api/app/sites/${domain}/purge`, { page: purgePath });
		purgePath = "";
	}

	async function resetStats() {
		if (resetConfirm !== "reset") throw new Error("Type reset to confirm.");
		await api(`/api/app/sites/${domain}/reset`, {});
		resetConfirm = "";
		imports = [];
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
	const sections = $derived([
		{ id: "general", label: "general" },
		{ id: "tracking", label: "tracking" },
		{ id: "collection", label: "collection" },
		{ id: "goals", label: "goals" },
		{ id: "funnels", label: "funnels" },
		...(segments?.length ? [{ id: "segments", label: "segments" }] : []),
		{ id: "imports", label: "imports" },
		...(google ? [{ id: "search-console", label: "search console" }] : []),
		{ id: "shields", label: "shields" },
		{ id: "sharing", label: "sharing" },
		{ id: "reports", label: "reports" },
		...(owner
			? [
					{ id: "guests", label: "guests" },
					{ id: "danger", label: "danger" }
				]
			: [])
	]);
</script>

<Dialog title="add goal" bind:open={adding}>
	<Form action="add goal" submit={addGoal}>
		<Toggle
			label="kind"
			bind:value={kind}
			options={[
				{ key: "event", label: "event" },
				{ key: "page", label: "page" },
				{ key: "revenue", label: "revenue" }
			]}
		/>
		<div class="grid gap-4 sm:grid-cols-2">
			<Field
				label={kind === "page" ? "path" : "event name"}
				placeholder={kind === "page" ? "/pricing" : "Signup"}
				bind:value
				required
			/>
			{#if kind === "page"}
				<Field
					label="scrolled at least"
					type="number"
					min="0"
					max="100"
					placeholder="0 for any visit"
					bind:value={scroll}
				/>
			{:else if kind === "revenue"}
				<Field
					label="currency"
					placeholder="EUR"
					maxlength={3}
					bind:value={currency}
					required
				/>
			{/if}
		</div>
		<Field label="display name" placeholder="optional" bind:value={goalName} />
	</Form>
</Dialog>

<Dialog
	title="rename goal"
	bind:open={
		() => renaming !== null,
		(open) => {
			if (!open) renaming = null;
		}
	}
>
	<Form action="rename" submit={renameGoal}>
		<Field label="name" bind:value={newName} required />
	</Form>
</Dialog>

<Dialog title="add funnel" bind:open={building}>
	<Form action="add funnel" submit={addFunnel}>
		<Field label="name" placeholder="Checkout" bind:value={funnelName} required />
		{#each steps as step, i (i)}
			<div class="flex items-end gap-2">
				<div class="flex-1">
					<Select
						label="step {i + 1}"
						value={step}
						options={(goals ?? []).map((goal) => ({ key: goal.id, label: goal.name }))}
						data
						pick={(id) => (steps[i] = id)}
					/>
				</div>
				{#if steps.length > 2}
					<Button
						type="button"
						variant="icon"
						aria-label="remove step"
						title="remove step"
						onclick={() => (steps = steps.filter((_, j) => j !== i))}
						><Icon svg={close} size={14} /></Button
					>
				{/if}
			</div>
		{/each}
		{#if steps.length < 8}
			<div>
				<Button
					type="button"
					variant="quiet"
					onclick={() => (steps = [...steps, goals?.[0]?.id ?? 0])}>add step</Button
				>
			</div>
		{/if}
	</Form>
</Dialog>

<Dialog title="add rule" bind:open={shielding}>
	<Form action="add rule" submit={addShield}>
		<Toggle label="kind" bind:value={shieldKind} options={KINDS} />
		<div class="flex items-end gap-2">
			<div class="flex-1">
				<Field
					label={KINDS.find((kind) => kind.key === shieldKind)?.meta ?? "value"}
					placeholder={KINDS.find((kind) => kind.key === shieldKind)?.example}
					bind:value={shieldValue}
					required
				/>
			</div>
			{#if shieldKind === "ip"}
				<Button type="button" variant="quiet" onclick={myIp}>use my ip</Button>
			{/if}
		</div>
	</Form>
</Dialog>

<Dialog title="add shared link" bind:open={linking}>
	<Form action="add link" submit={addLink}>
		<Field label="name" placeholder="Investors" bind:value={linkName} required />
		<Field label="password" type="password" placeholder="optional" bind:value={linkPassword} />
	</Form>
</Dialog>

<Dialog title="add report" bind:open={reporting}>
	<Form action="add report" submit={addReport}>
		<Toggle label="kind" bind:value={reportKind} options={REPORTS} />
		<div class="grid gap-4 sm:grid-cols-2">
			<Field label="send to" type="email" bind:value={reportEmail} required />
			{#if reportKind === "spike" || reportKind === "drop"}
				<Field
					label={reportKind === "spike"
						? "current visitors at least"
						: "visitors a day under"}
					type="number"
					min="1"
					bind:value={reportThreshold}
					required
				/>
			{/if}
		</div>
	</Form>
</Dialog>

<Shell>
	{#if site}
		<Settings {sections}>
			{#snippet title()}
				<Title>
					<a
						href={resolve("/[site]", { site: domain })}
						class="flex items-center gap-3 transition-colors duration-100 hover:text-cyan"
					>
						<Glyph spec={icon("event:hostname", domain)} fallback={domain} />
						{domain}
					</a>
				</Title>
			{/snippet}

			<Section id="general" label="general">
				<Form action="rename" submit={renameSite} inline>
					<Field label="domain" bind:value={domainName} required />
				</Form>
				<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
					<Select
						label="keep data"
						value={site.retention}
						options={RETENTION}
						pick={setRetention}
					/>
					<Select
						label="timezone"
						value={site.timezone}
						options={ZONES}
						data
						pick={setTimezone}
					/>
					{#if owner && home && targets.length > 1}
						<Select label="team" value={home.id} options={targets} pick={moveTo} />
					{/if}
				</div>
			</Section>

			<Section id="tracking" label="tracking">
				<Snippet {domain} />
			</Section>

			<Section id="collection" label="collection">
				<span class="text-[14px] text-dim"
					>Unchecked data is never stored for new visits. Without sessions every pageview
					counts as a new visitor.</span
				>
				<Checks
					bind:value={
						() =>
							COLLECTED.map((item) => item.key).filter(
								(key) => !site?.skip.includes(key)
							),
						setCollected
					}
					options={COLLECTED}
				/>
			</Section>

			<Section id="goals" label="goals">
				{#snippet actions()}
					<Button variant="quiet" onclick={() => (adding = true)}>add goal</Button>
				{/snippet}
				{#if goals?.length === 0}
					<span class="text-[14px] text-dim">No goals yet.</span>
				{/if}
				<List
					items={goals?.map((goal) => ({
						key: goal.id,
						title: goal.name,
						meta: [
							goal.currency
								? `revenue in ${goal.currency}`
								: goal.page
									? "page"
									: "event",
							...(goal.value !== goal.name ? [goal.value] : []),
							...(goal.scroll ? [`scrolled ${goal.scroll}%`] : [])
						]
					})) ?? null}
					remove={removeGoal}
				>
					{#snippet control(key)}
						{@const goal = goals?.find((goal) => goal.id === key)}
						{#if goal}
							<Button
								variant="plain"
								onclick={() => ((newName = goal.name), (renaming = goal))}
								>rename</Button
							>
						{/if}
					{/snippet}
				</List>
			</Section>

			<Section id="funnels" label="funnels">
				{#snippet actions()}
					{#if (goals?.length ?? 0) >= 2}
						<Button variant="quiet" onclick={startFunnel}>add funnel</Button>
					{/if}
				{/snippet}
				{#if (goals?.length ?? 0) < 2}
					<span class="text-[14px] text-dim"
						>Add two goals to build a funnel from them.</span
					>
				{:else if funnels?.length === 0}
					<span class="text-[14px] text-dim">No funnels yet.</span>
				{/if}
				<List
					items={funnels?.map((funnel) => ({
						key: funnel.id,
						title: funnel.name,
						meta: funnel.steps.map(
							(id, i) =>
								`${i + 1}. ${goals?.find((goal) => goal.id === id)?.name ?? "?"}`
						)
					})) ?? null}
					remove={removeFunnel}
				/>
			</Section>

			{#if segments?.length}
				<Section id="segments" label="segments">
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

			<Section id="imports" label="imports">
				<List
					items={imports?.map((item) => ({
						key: item.id,
						title: `${item.first} to ${item.last}`,
						meta: [item.source]
					})) ?? null}
					remove={removeImport}
				/>
				<Form action="import" submit={runImport} inline>
					<Upload bind:file accept={source === "goatcounter" ? ".csv,.gz" : ".zip"} />
					<Toggle
						bind:value={source}
						options={[
							{ key: "impactite", label: "impactite" },
							{ key: "plausible", label: "plausible" },
							{ key: "goatcounter", label: "goatcounter" }
						]}
					/>
				</Form>
				<Message tone="cyan">{imported}</Message>
			</Section>

			{#if google}
				<Section id="search-console" label="search console">
					{#snippet actions()}
						{#if google?.connected}
							<Button variant="quiet" onclick={disconnectGoogle}>disconnect</Button>
						{:else if google?.configured}
							<Button
								variant="quiet"
								onclick={() =>
									(location.href = `/api/app/sites/${domain}/google/connect`)}
								>connect</Button
							>
						{/if}
					{/snippet}
					{#if googleFailed && !google.connected}
						<Message tone="bad"
							>Google did not accept the sign in. Try to connect again.</Message
						>
					{/if}
					{#if !google.configured}
						<span class="text-[14px] text-dim"
							>To show Google search terms, the server admin creates a Google OAuth
							client with this redirect URI and sets IMPACTITE_GOOGLE_CLIENT_ID and
							IMPACTITE_GOOGLE_CLIENT_SECRET.</span
						>
						<Code code={google.redirect} />
					{:else if !google.connected}
						<span class="text-[14px] text-dim"
							>Connect Google Search Console to see search terms when you filter by
							the Google source.</span
						>
					{:else if google.properties.length}
						<Select
							label="property"
							value={google.property}
							options={google.properties.map((property) => ({
								key: property,
								label: property
							}))}
							pick={chooseProperty}
							text={google.property || "choose a property"}
							data
						/>
					{:else}
						<span class="text-[14px] text-dim"
							>This Google account has no Search Console properties.</span
						>
					{/if}
				</Section>
			{/if}

			<Section id="shields" label="shields">
				{#snippet actions()}
					<Button variant="quiet" onclick={() => (shielding = true)}>add rule</Button>
				{/snippet}
				{#if shields?.length === 0}
					<span class="text-[14px] text-dim">Every visit counts.</span>
				{/if}
				<List
					items={shields?.map((rule) => ({
						key: rule.id,
						title: rule.value,
						meta: [KINDS.find((kind) => kind.key === rule.kind)?.meta ?? rule.kind]
					})) ?? null}
					remove={removeShield}
				/>
				<span class="text-[14px] text-dim"
					>To skip your own browser, run this in its console on your site.</span
				>
				<Code code={IGNORE} />
			</Section>

			<Section id="sharing" label="sharing">
				{#snippet actions()}
					<Button variant="quiet" onclick={() => (linking = true)}>add link</Button>
				{/snippet}
				<div class="flex items-center justify-between gap-4">
					<span class="text-[14px] text-dim"
						>Anyone can view the dashboard at {url(domain)}</span
					>
					<Toggle
						bind:value={
							() => (publicLink ? "public" : "private"),
							(mode: "private" | "public") => setPublic(mode === "public")
						}
						options={[
							{ key: "private", label: "private" },
							{ key: "public", label: "public" }
						]}
					/>
				</div>
				<List
					items={links
						?.filter((link) => link.slug !== domain)
						.map((link) => ({
							key: link.id,
							title: link.name,
							meta: [url(link.slug), ...(link.locked ? ["password"] : [])]
						})) ?? null}
					remove={removeLink}
				>
					{#snippet control(key)}
						{@const link = links?.find((link) => link.id === key)}
						{#if link}
							<Button
								variant="plain"
								onclick={() => navigator.clipboard.writeText(url(link.slug))}
								>copy</Button
							>
						{/if}
					{/snippet}
				</List>
				{#if embedLink}
					<span class="text-[14px] text-dim">Embed the dashboard in another page.</span>
					<Code
						code={`<iframe src="${url(embedLink.slug)}?embed=true" width="100%" height="1600" style="border:0" loading="lazy"></iframe>`}
					/>
				{/if}
				<div class="flex items-center justify-between gap-4">
					<span class="text-[14px] text-dim">Pages can show their visitor count.</span>
					<Toggle
						bind:value={
							() => (site?.counter ? "on" : "off"),
							(mode: "off" | "on") => setCounter(mode === "on")
						}
						options={[
							{ key: "off", label: "off" },
							{ key: "on", label: "on" }
						]}
					/>
				</div>
				{#if site?.counter}
					<Code code={counterSnippet} />
				{/if}
			</Section>

			<Section id="reports" label="reports">
				{#snippet actions()}
					<Button variant="quiet" onclick={() => (reporting = true)}>add report</Button>
				{/snippet}
				{#if reports?.length === 0}
					<span class="text-[14px] text-dim">No reports or alerts yet.</span>
				{/if}
				<List
					items={reports?.map((report) => ({
						key: report.id,
						title: report.email,
						meta: [
							REPORTS.find((kind) => kind.key === report.kind)?.meta(
								report.threshold
							) ?? report.kind
						]
					})) ?? null}
					remove={removeReport}
				/>
			</Section>

			{#if owner && people}
				<Section id="guests" label="guests">
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
			{/if}

			{#if owner}
				<Section id="danger" label="danger" danger>
					<Form action="remove page" submit={purgePage} inline danger>
						<Field
							label="remove all stats of one page, imported days stay"
							placeholder="/old-page"
							bind:value={purgePath}
						/>
					</Form>
					<Form action="reset stats" submit={resetStats} inline danger>
						<Field
							label="type reset to remove all stats but keep the site and its settings"
							bind:value={resetConfirm}
						/>
					</Form>
					<Form action="delete site" submit={destroy} inline danger>
						<Field
							label="type {domain} to delete it and all its data"
							bind:value={confirm}
						/>
					</Form>
				</Section>
			{/if}
		</Settings>
	{/if}
</Shell>
