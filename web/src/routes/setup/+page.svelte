<script lang="ts">
	import { page } from "$app/state";
	import NewAccount from "#lib/NewAccount.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	const token = page.url.searchParams.get("token") ?? "";
</script>

<Gate>
	{#if token}
		<NewAccount {token} />
	{:else}
		<Message>Open the setup link from the server log.</Message>
	{/if}
</Gate>
