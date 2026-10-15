<script lang="ts">
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { refresh } from "#lib/session.svelte.ts";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	let result = $state("");
	let failed = $state(false);

	$effect(() => {
		api<{ email: string }>("/api/app/me/email", { token: page.params.token })
			.then(async ({ email }) => {
				result = `Your email is now ${email}.`;
				await refresh().catch(() => {});
			})
			.catch((err) => {
				failed = true;
				result = err.message;
			});
	});
</script>

<Gate>
	<Message tone={failed ? "bad" : result ? "cyan" : "dim"}
		>{result || "Confirming your email."}</Message
	>
</Gate>
