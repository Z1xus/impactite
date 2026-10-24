<script lang="ts">
	import { api, get } from "#lib/api.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import { sitePath } from "./site.ts";

	let { domain }: { domain: string } = $props();

	type Google = {
		configured: boolean;
		redirect: string;
		connected: boolean;
		property: string;
		properties: string[];
	};

	const path = $derived(`${sitePath(domain)}/google`);
	let google = $state<Google | null>(null);
	const failed = new URLSearchParams(location.search).get("google") === "failed";

	$effect(() => {
		get<Google>(path).then((result) => (google = result));
	});

	const choose = (property: string) =>
		attempt(async () => {
			await api(path, { property }, "PATCH");
			if (google) google.property = property;
		});

	const disconnect = () =>
		attempt(
			async () => {
				await api(path, undefined, "DELETE");
				google = await api<Google>(path);
			},
			{
				title: "Disconnect Search Console?",
				body: "Search terms won't appear on your dashboard until you connect again.",
				action: "Disconnect"
			}
		);
</script>

<Section id="search-console" label="search console">
	{#snippet actions()}
		{#if google?.connected}
			<Button variant="quiet" onclick={disconnect}>Disconnect</Button>
		{:else if google?.configured}
			<Button variant="quiet" onclick={() => (location.href = `${path}/connect`)}
				>Connect</Button
			>
		{/if}
	{/snippet}
	{#if google}
		{#if failed && !google.connected}
			<Message tone="bad">Google didn't accept the sign-in. Try connecting again.</Message>
		{/if}
		{#if !google.configured}
			<Note>
				Set IMPACTITE_GOOGLE_CLIENT_ID and IMPACTITE_GOOGLE_CLIENT_SECRET with this redirect
				URI.
			</Note>
			<Code code={google.redirect} />
		{:else if !google.connected}
			<Note>Not connected.</Note>
		{:else if google.properties.length}
			<Select
				label="property"
				value={google.property}
				options={google.properties.map((property) => ({ key: property, label: property }))}
				pick={choose}
				text={google.property || "choose a property"}
				data
			/>
		{:else}
			<Note>No properties on this Google account.</Note>
		{/if}
	{:else}
		<Skeleton class="h-control w-full" />
	{/if}
</Section>
