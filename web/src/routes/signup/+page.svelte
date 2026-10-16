<script lang="ts">
	import NewAccount from "#lib/NewAccount.svelte";
	import { session } from "#lib/session.svelte.ts";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";
</script>

<Gate>
	{#if session.signup}
		<NewAccount />
	{:else}
		<Message>This server is invite only.</Message>
	{/if}
</Gate>
