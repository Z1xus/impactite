<script lang="ts">
	import { goto, preloadCode } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { absorb } from "#lib/art/absorb.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	const slug = $derived(page.params.slug ?? "");
	let panel = $state<HTMLElement>();

	const target = $derived(resolve("/share/[slug]", { slug }));

	// load the dashboard code and the share while the form is still on screen
	$effect(() => {
		void preloadCode("/share/[slug]");
	});

	async function enter() {
		const share = api(`/api/app/shares/${slug}`, { password: "" })
			.then((opened) => sessionStorage.setItem(`share:${slug}`, JSON.stringify(opened)))
			.catch(() => {});
		await Promise.all([panel && absorb(panel), share]);
		await goto(target);
	}
</script>

<Gate>
	<div class="flex flex-col gap-3" bind:this={panel}>
		<Form block action="enter the demo" submit={enter}>
			<Field label="email" value="demo@impactite.dev" readonly />
			<Field label="password" type="password" value="impactite-demo" readonly />
		</Form>
		<Message>Read-only dashboard of {slug}.</Message>
	</div>
</Gate>
