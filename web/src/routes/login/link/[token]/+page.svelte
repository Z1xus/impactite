<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { onMount } from "svelte";
	import { api, type Me } from "#lib/api.ts";
	import { adopt } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	let error = $state("");

	onMount(async () => {
		try {
			const { me } = await api<{ me: Me }>("/api/app/login/link/accept", {
				token: page.params.token
			});
			adopt(me);
			await goto(resolve("/"));
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		}
	});
</script>

<svelte:head>
	<title>Sign in · impactite</title>
</svelte:head>

<Gate>
	<div class="flex flex-col items-center gap-4">
		<Message tone={error ? "bad" : "dim"}>{error || "Signing you in."}</Message>
		{#if error}
			<Button variant="quiet" href={resolve("/login")}>Sign in</Button>
		{/if}
	</div>
</Gate>
