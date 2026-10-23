<script lang="ts">
	import { page } from "$app/state";
	import { ApiError, access, api } from "#lib/api.ts";
	import Empty from "#lib/ui/Empty.svelte";
	import Failed from "#lib/ui/Failed.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import { icon } from "#lib/icons/resolve.ts";
	import { wear } from "#lib/theme.svelte.ts";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Logo from "#lib/ui/Logo.svelte";
	import Status from "#lib/uptime/Status.svelte";
	import type { Status as Report } from "#lib/uptime/types.ts";

	const slug = $derived(page.params.slug ?? "");
	const embed = $derived(page.url.searchParams.get("embed") === "true");

	let report = $state.raw<Report | null>(null);
	let locked = $state(false);
	let missing = $state(false);
	let failed = $state(false);
	let password = $state("");
	let timer: ReturnType<typeof setTimeout> | undefined;

	const ready = $derived(report !== null);
	const look = $derived(report?.look);
	const home = $derived.by(() => {
		try {
			return look?.home ? new URL(look.home).hostname : "";
		} catch {
			return "";
		}
	});

	$effect(() => {
		if (look) wear(look);
	});

	$effect(() => () => wear(null));
	const url = $derived(`/api/app/status/${slug}`);

	function fail(err: unknown) {
		if (err instanceof ApiError && err.status === 401) {
			sessionStorage.removeItem(`status:${slug}`);
			access.share = "";
			locked = true;
		} else if (err instanceof ApiError && err.status === 404) missing = true;
		else if (!report) failed = true;
	}

	async function load() {
		failed = false;
		const next = await api<Report>(url).catch((err) => (fail(err), report));
		if (JSON.stringify(next) !== JSON.stringify(report)) report = next;
	}

	async function unlock(secret: string) {
		const opened = await api<Report>(url, { password: secret });
		access.share = opened.token ?? "";
		sessionStorage.setItem(`status:${slug}`, access.share);
		locked = false;
		report = opened;
	}

	$effect(() => {
		const early = window.opened as (Report & { locked?: boolean }) | undefined;
		window.opened = undefined;
		access.share = sessionStorage.getItem(`status:${slug}`) ?? "";
		if (early && !early.locked) report = early;
		else if (early && !access.share) locked = true;
		else load();
	});

	$effect(() => {
		if (!ready) return;
		const source = new EventSource(
			`${url}/live${access.share ? `?share=${encodeURIComponent(access.share)}` : ""}`
		);
		source.addEventListener("change", (event) => {
			const items: { id: number; state: string }[] = JSON.parse(event.data);
			const stale = items.some(
				(item) =>
					report?.monitors.find((known) => known.id === item.id)?.state !== item.state
			);
			if (!stale) return;
			clearTimeout(timer);
			timer = setTimeout(load, 800);
		});
		const refresh = setInterval(load, 60_000);
		return () => {
			source.close();
			clearInterval(refresh);
			clearTimeout(timer);
		};
	});

	$effect(() => {
		if (page.url.searchParams.get("background") !== "transparent") return;
		document.documentElement.style.background = document.body.style.background = "transparent";
	});
</script>

<svelte:head>
	<title>{report?.name || "Status"}</title>
</svelte:head>

{#snippet body()}
	{#if report}
		<Status status={report} />
	{:else if failed}
		<Empty retry={load} />
	{:else}
		<div class="flex flex-col gap-6">
			<Skeleton class="h-6 w-48" />
			<Skeleton class="h-14 w-full" />
			<Skeleton class="h-48 w-full" />
		</div>
	{/if}
{/snippet}

{#if locked}
	<Gate>
		<Form block action="Open page" submit={() => unlock(password)}>
			<Field label="password" type="password" bind:value={password} autofocus required />
		</Form>
	</Gate>
{:else if missing}
	<Failed text="This page doesn't exist." />
{:else if embed}
	<div class="p-4">{@render body()}</div>
{:else if look && !look.brand && !home}
	<main class="mx-auto max-w-page gutter py-8">{@render body()}</main>
{:else}
	<Shell>
		{#snippet brand()}
			{#if home}
				<!-- eslint-disable svelte/no-navigation-without-resolve -- the page owner's website -->
				<a
					href={look?.home}
					class="-ml-2 flex h-control items-center gap-2 px-2 text-body text-fg"
				>
					<Glyph spec={icon("event:hostname", home)} fallback={home} />
					{home}
				</a>
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{:else}
				<a
					href="https://impactite.dev"
					class="-ml-2 flex h-control items-center gap-2 px-2 font-mono text-body text-fg"
				>
					<Logo />
					impactite
				</a>
			{/if}
		{/snippet}
		{@render body()}
	</Shell>
{/if}
