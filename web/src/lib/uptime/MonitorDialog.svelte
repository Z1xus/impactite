<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import globe from "pixelarticons/svg/globe.svg?raw";
	import heart from "pixelarticons/svg/heart.svg?raw";
	import server from "pixelarticons/svg/server.svg?raw";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Copy from "#lib/ui/Copy.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Disclosure from "#lib/ui/Disclosure.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Pick from "#lib/ui/Pick.svelte";
	import Select from "#lib/ui/Select.svelte";
	import State from "#lib/ui/State.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { every, hostOf, latency } from "./format.ts";
	import { BEATS, INTERVALS, type Config, type Health, type Monitor } from "./types.ts";

	let {
		open = $bindable(false),
		team,
		monitor,
		saved
	}: {
		open: boolean;
		team: number;
		monitor?: Monitor & { config: Config };
		saved?: () => unknown;
	} = $props();

	type Kind = "website" | "port" | "heartbeat";
	type Result = {
		state: Health;
		latency: number | null;
		cause: string | null;
		cert_expires: number | null;
	};

	const KINDS: { key: Kind; label: string; svg: string; example: string }[] = [
		{ key: "website", label: "Website", svg: globe, example: "example.com" },
		{ key: "port", label: "Port", svg: server, example: "db.example.com:5432" },
		{ key: "heartbeat", label: "Cron job", svg: heart, example: "" }
	];
	const METHODS = ["GET", "HEAD", "POST"].map((key) => ({ key, label: key }));

	let picked = $state<Kind>("website");
	let target = $state("");
	let name = $state("");
	let interval = $state(60);
	let method = $state("GET");
	let status = $state("200-399");
	let body = $state<"contains" | "lacks">("contains");
	let text = $state("");
	let headers = $state("");
	let timeout = $state("10");
	let slow = $state("2000");
	let grace = $state("60");
	let created = $state<{ id: number; url: string } | null>(null);
	let testing = $state(false);
	let result = $state.raw<Result | null>(null);
	let failure = $state("");

	const typed = $derived(
		/^[^/:\s]+:\d+$/.test(target.trim()) && !/^https?:/i.test(target.trim()) ? "port" : null
	);
	const kind = $derived<Kind>(
		monitor
			? monitor.kind === "heartbeat"
				? "heartbeat"
				: monitor.kind === "tcp"
					? "port"
					: "website"
			: picked === "heartbeat"
				? "heartbeat"
				: (typed ?? (/^https?:/i.test(target.trim()) ? "website" : picked))
	);
	const host = $derived(hostOf(target.trim()));
	const url = $derived.by(() => {
		const raw = target.trim();
		if (!raw || kind !== "website") return "";
		try {
			return new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`).href;
		} catch {
			return "";
		}
	});
	const watched = $derived(
		kind === "website"
			? url
				? `${method} ${url}`
				: ""
			: kind === "port" && typed
				? target.trim()
				: ""
	);
	const intervals = $derived(
		(kind === "heartbeat" ? BEATS : INTERVALS).map((key) => ({ key, label: every(key) }))
	);

	function reset() {
		created = null;
		result = null;
		failure = "";
		picked = "website";
		target = monitor?.kind === "heartbeat" ? "" : (monitor?.target ?? "");
		name = monitor?.name ?? "";
		interval = monitor?.interval ?? 60;
		const config = monitor?.config;
		method = config?.method ?? "GET";
		status = config?.status ?? "200-399";
		body = config?.lacks && !config.contains ? "lacks" : "contains";
		text = (body === "lacks" ? config?.lacks : config?.contains) ?? "";
		headers = Object.entries(config?.headers ?? {})
			.map(([key, value]) => `${key}: ${value}`)
			.join("\n");
		timeout = String(config?.timeout ?? 10);
		slow = String(config?.slow ?? 2000);
		grace = String(config?.grace ?? 60);
	}

	$effect(() => {
		if (open) untrack(reset);
	});

	$effect(() => {
		void watched;
		result = null;
		failure = "";
	});

	$effect(() => {
		if (kind === "heartbeat" && !monitor && interval === 60) interval = 3600;
		if (kind !== "heartbeat" && !monitor && !INTERVALS.includes(interval)) interval = 60;
	});

	function fields() {
		const lines = headers
			.split("\n")
			.map((line) => [
				line.slice(0, line.indexOf(":")).trim(),
				line.slice(line.indexOf(":") + 1).trim()
			])
			.filter(([key, value]) => key && value);
		return {
			name: name.trim() || undefined,
			interval,
			...(kind === "heartbeat"
				? { grace: Number(grace) }
				: kind === "port"
					? { timeout: Number(timeout) }
					: {
							timeout: Number(timeout),
							method,
							status,
							contains: body === "contains" ? text : "",
							lacks: body === "lacks" ? text : "",
							headers: Object.fromEntries(lines),
							slow: Number(slow) || undefined
						})
		};
	}

	async function test() {
		testing = true;
		failure = "";
		try {
			result = await api<Result>("/api/app/uptime/probe", {
				team,
				target: target.trim(),
				...fields()
			});
		} catch (err) {
			failure = err instanceof Error ? err.message : "The check didn't run.";
		} finally {
			testing = false;
		}
	}

	async function submit() {
		if (monitor) {
			await api(
				`/api/app/uptime/${monitor.id}`,
				{ target: kind === "heartbeat" ? undefined : target.trim(), ...fields() },
				"PATCH"
			);
			open = false;
			await saved?.();
			return;
		}
		const made = await api<{ id: number; target: string }>("/api/app/uptime", {
			team,
			kind: kind === "heartbeat" ? "heartbeat" : undefined,
			target: kind === "heartbeat" ? undefined : target.trim(),
			...fields()
		});
		if (kind === "heartbeat") {
			created = { id: made.id, url: made.target };
			return;
		}
		await api(`/api/app/uptime/${made.id}/check`, {}).catch(() => {});
		await finish(made.id);
	}

	async function finish(id: number) {
		open = false;
		await goto(resolve("/uptime/[id]", { id: String(id) }));
	}
</script>

<Dialog title={monitor ? "edit monitor" : "add monitor"} bind:open>
	{#if created}
		<div class="flex flex-col gap-4">
			<span class="label">call this after each run</span>
			<div class="flex items-center gap-2 rounded-px border border-control py-1 pr-1 pl-3">
				<span class="min-w-0 flex-1 truncate num text-small text-fg">{created.url}</span>
				<Copy text={created.url} />
			</div>
			<span class="label">in a script</span>
			<Code code="your-job && curl -fsS -m 10 --retry 3 {created.url}" />
			<span class="label">in crontab</span>
			<Code code="0 3 * * * /path/to/job && curl -fsS -m 10 --retry 3 {created.url}" />
			<Button class="self-end" onclick={() => finish(created!.id)}>Done</Button>
		</div>
	{:else}
		<Form
			action={monitor ? "Save" : "Add monitor"}
			done={monitor ? "Saved." : "Added."}
			{submit}
		>
			{#if !monitor}
				<div class="flex flex-wrap gap-1.5" role="group" aria-label="kind">
					{#each KINDS as item (item.key)}
						<Pick
							spec={{ kind: "pixel", svg: item.svg }}
							label={item.label}
							on={kind === item.key}
							dim={!!target.trim() && kind !== item.key}
							onclick={() => (picked = item.key)}
						/>
					{/each}
				</div>
			{/if}
			{#if kind === "heartbeat"}
				<Field
					label="name"
					placeholder="Nightly backup"
					maxlength={80}
					autofocus
					bind:value={name}
					required
				/>
			{:else}
				<div class="flex flex-col gap-2">
					<Field
						label={kind === "port" ? "host and port" : "address"}
						placeholder={KINDS.find((item) => item.key === kind)?.example}
						autocomplete="off"
						autocapitalize="off"
						spellcheck={false}
						autofocus
						bind:value={target}
						required
					>
						{#snippet lead()}
							{#if kind === "website" && host}
								<Glyph spec={icon("event:hostname", host)} fallback={host} />
							{:else}
								<Glyph
									spec={{ kind: "pixel", svg: kind === "port" ? server : globe }}
								/>
							{/if}
						{/snippet}
					</Field>
					{#if watched}
						<div
							class="flex min-h-control items-center gap-3 rounded-px bg-raised py-1 pr-1 pl-3"
						>
							<span class="min-w-0 flex-1 truncate meta">
								{#if failure}
									<span class="text-bad">{failure}</span>
								{:else if result}
									<span class="flex items-center gap-3">
										<State state={result.state} />
										<span class="truncate">
											{result.cause ??
												(result.latency ? latency(result.latency) : "")}
											{#if result.cert_expires}
												<span class="ml-2"
													>certificate {Math.max(
														0,
														Math.round(
															(result.cert_expires -
																Date.now() / 1000) /
																86_400
														)
													)} days</span
												>
											{/if}
										</span>
									</span>
								{:else}
									{watched}
								{/if}
							</span>
							<Button
								type="button"
								variant="quiet"
								size="sm"
								busy={testing}
								onclick={test}>Test</Button
							>
						</div>
					{/if}
				</div>
			{/if}
			<div class="grid gap-4 sm:grid-cols-2">
				{#if kind !== "heartbeat"}
					<Field
						label="name"
						placeholder={host || "Name"}
						maxlength={80}
						bind:value={name}
					/>
				{/if}
				<Select
					label={kind === "heartbeat" ? "runs every" : "check every"}
					value={interval}
					options={intervals}
					pick={(key) => (interval = key)}
				/>
				{#if kind === "heartbeat"}
					<Field
						label="allowed delay"
						type="number"
						min="0"
						unit="s"
						bind:value={grace}
					/>
				{/if}
			</div>
			{#if kind !== "heartbeat"}
				<Disclosure label="Options">
					{#if kind === "website"}
						<div class="grid gap-4 sm:grid-cols-2">
							<Select
								label="method"
								value={method}
								options={METHODS}
								pick={(key) => (method = key)}
							/>
							<Field
								label="ok status codes"
								placeholder="200-399"
								bind:value={status}
							/>
						</div>
						<div class="flex flex-col gap-2">
							<span class="label">response must</span>
							<div class="flex flex-wrap items-center gap-2">
								<Toggle
									bind:value={body}
									options={[
										{ key: "contains", label: "Contain" },
										{ key: "lacks", label: "Not contain" }
									]}
								/>
								<div class="min-w-48 flex-1">
									<Field
										label="text"
										quiet
										placeholder="Text, optional"
										bind:value={text}
									/>
								</div>
							</div>
						</div>
						<Field
							label="request headers"
							rows={3}
							placeholder="Authorization: Bearer token"
							bind:value={headers}
						/>
					{/if}
					<div class="grid gap-4 sm:grid-cols-2">
						<Field
							label="give up after"
							type="number"
							min="1"
							max="60"
							unit="s"
							bind:value={timeout}
						/>
						{#if kind === "website"}
							<Field
								label="slow after"
								type="number"
								min="1"
								unit="ms"
								bind:value={slow}
							/>
						{/if}
					</div>
				</Disclosure>
			{/if}
		</Form>
	{/if}
</Dialog>
