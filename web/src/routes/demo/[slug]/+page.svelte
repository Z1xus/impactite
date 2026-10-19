<script lang="ts">
	import { goto, preloadCode } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { absorb } from "#lib/art/absorb.ts";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";

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

<svelte:head>
	<title>Demo | impactite</title>
</svelte:head>

<Gate>
	<div bind:this={panel}>
		<Form block action="Open demo" submit={enter} />
	</div>
</Gate>
