<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api, type Me } from "#lib/api.ts";
	import { adopt } from "#lib/session.svelte.ts";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	let error = $state("");

	$effect(() => {
		api<{ me: Me }>("/api/app/login/link/accept", { token: page.params.token })
			.then(({ me }) => {
				adopt(me);
				return goto(resolve("/"));
			})
			.catch((err) => (error = err.message));
	});
</script>

<Gate>
	<Message tone={error ? "bad" : "dim"}>{error || "Signing you in."}</Message>
</Gate>
