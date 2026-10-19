<script lang="ts">
	import { resolve } from "$app/paths";
	import NewAccount from "#lib/NewAccount.svelte";
	import { session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";
</script>

<svelte:head>
	<title>Sign up | impactite</title>
</svelte:head>

<Gate>
	{#if session.signup}
		<NewAccount />
	{:else}
		<div class="flex flex-col items-center gap-4">
			<Message>This server is invite only.</Message>
			<Button variant="quiet" href={resolve("/login")}>Sign in</Button>
		</div>
	{/if}
</Gate>
