<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { onMount } from "svelte";
	import { api, ApiError, type Me } from "#lib/api.ts";
	import { adopt } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	let error = $state("");
	let asked = $state(false);
	let code = $state("");

	async function accept() {
		const { me } = await api<{ me: Me }>("/api/app/login/link/accept", {
			token: page.params.token,
			...(asked && { code })
		});
		adopt(me);
		await goto(resolve("/"));
	}

	onMount(async () => {
		try {
			await accept();
		} catch (err) {
			if (err instanceof ApiError && err.data.totp) asked = true;
			else error = err instanceof Error ? err.message : "Something went wrong.";
		}
	});
</script>

<svelte:head>
	<title>Sign in to impactite</title>
</svelte:head>

<Gate>
	{#if asked}
		<Form block action="Sign in" submit={accept}>
			<Field
				label="code"
				placeholder="App or recovery code"
				bind:value={code}
				autocomplete="one-time-code"
				autofocus
				required
			/>
		</Form>
	{:else}
		<div class="flex flex-col items-center gap-4">
			<Message tone={error ? "bad" : "dim"}>{error || "Signing you in."}</Message>
			{#if error}
				<Button variant="quiet" href={resolve("/login")}>Sign in</Button>
			{/if}
		</div>
	{/if}
</Gate>
