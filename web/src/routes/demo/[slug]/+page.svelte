<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { absorb } from "#lib/art/absorb.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	const slug = $derived(page.params.slug ?? "");
	let panel = $state<HTMLElement>();

	async function enter() {
		if (panel) await absorb(panel);
		await goto(resolve("/share/[slug]", { slug }));
	}
</script>

<Gate>
	<div class="flex flex-col gap-3" bind:this={panel}>
		<Form block action="enter the demo" submit={enter}>
			<Field label="email" value="demo@impactite.dev" readonly />
			<Field label="password" type="password" value="impactite-demo" readonly />
		</Form>
		<Message
			>This is the live dashboard of {slug}. You can look around, nothing can be changed.</Message
		>
	</div>
</Gate>
