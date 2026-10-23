<script lang="ts">
	import { page } from "$app/state";
	import { ApiError, access, api } from "#lib/api.ts";
	import Dashboard from "#lib/dashboard/Dashboard.svelte";
	import Failed from "#lib/ui/Failed.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import { query } from "#lib/stats.ts";
	import type { Share } from "#lib/session.svelte.ts";

	const slug = $derived(page.params.slug ?? "");
	const embed = $derived(page.url.searchParams.get("embed") === "true");

	let share = $state<Share | null>(null);
	let locked = $state(false);
	let missing = $state("");
	let password = $state("");

	async function unlock(secret: string) {
		const opened = await api<Share>(`/api/app/shares/${slug}`, { password: secret });
		access.share = opened.token;
		sessionStorage.setItem(`share:${slug}`, JSON.stringify(opened));
		share = opened;
	}

	function open() {
		unlock("").catch((err) => {
			if (err instanceof ApiError && err.status === 401) locked = true;
			else missing = "This link doesn't exist.";
		});
	}

	$effect(() => {
		const early = window.opened as Share | undefined;
		window.opened = undefined;
		if (early) {
			access.share = early.token;
			share = early;
			return;
		}
		const key = `share:${slug}`;
		const saved: Share | null = JSON.parse(sessionStorage.getItem(key) ?? "null");
		if (!saved) return open();
		access.share = saved.token;
		// a saved token stops working when the link is removed or its password changes
		query(saved.domain, { metrics: ["visitors"], date_range: "day" }).then(
			() => (share = saved),
			() => {
				sessionStorage.removeItem(key);
				access.share = "";
				open();
			}
		);
	});

	$effect(() => {
		if (page.url.searchParams.get("background") !== "transparent") return;
		document.documentElement.style.background = document.body.style.background = "transparent";
	});
</script>

<svelte:head>
	<title>{share?.name ?? "Shared dashboard"}</title>
</svelte:head>

{#if share}
	{#if embed}
		<div class="p-4">
			<Dashboard domain={share.domain} timezone={share.timezone} readonly />
		</div>
	{:else}
		<Shell>
			<Dashboard domain={share.domain} timezone={share.timezone} readonly />
		</Shell>
	{/if}
{:else if locked}
	<Gate>
		<Form block action="Open dashboard" submit={() => unlock(password)}>
			<Field label="password" type="password" bind:value={password} autofocus required />
		</Form>
	</Gate>
{:else if missing}
	<Failed text={missing} />
{:else if !embed}
	<Gate />
{/if}
