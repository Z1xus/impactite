<script lang="ts">
	import { page } from "$app/state";
	import { ApiError, access, api } from "#lib/api.ts";
	import Dashboard from "#lib/dashboard/Dashboard.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import { query } from "#lib/stats.ts";

	type Share = { domain: string; timezone: string; name: string; token: string };

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
			else missing = err instanceof Error ? err.message : "This link does not exist.";
		});
	}

	$effect(() => {
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
		<Form block action="open dashboard" submit={() => unlock(password)}>
			<Field label="password" type="password" bind:value={password} required />
		</Form>
	</Gate>
{:else if missing}
	<Gate>
		<Message tone="bad">{missing}</Message>
	</Gate>
{/if}
