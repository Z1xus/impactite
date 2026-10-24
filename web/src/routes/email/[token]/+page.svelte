<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { onMount } from "svelte";
	import { api } from "#lib/api.ts";
	import { refresh } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	let result = $state("");
	let failed = $state(false);

	onMount(async () => {
		try {
			const { email } = await api<{ email: string }>("/api/app/me/email", {
				token: page.params.token
			});
			result = `Your email is now ${email}.`;
			await refresh().catch(() => {});
		} catch (err) {
			failed = true;
			result = err instanceof Error ? err.message : "Something went wrong.";
		}
	});
</script>

<svelte:head>
	<title>Confirm your email</title>
</svelte:head>

<Gate>
	<div class="flex flex-col items-center gap-4">
		<Message tone={failed ? "bad" : result ? "cyan" : "dim"}
			>{result || "Confirming your email…"}</Message
		>
		{#if result}
			<Button variant="quiet" href={resolve("/")}>Continue</Button>
		{/if}
	</div>
</Gate>
