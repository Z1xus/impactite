<script lang="ts">
	import Checks from "#lib/ui/Checks.svelte";
	import Code from "#lib/ui/Code.svelte";

	let { domain }: { domain: string } = $props();

	let features = $state<string[]>([]);

	const order = [
		"hash",
		"outbound-links",
		"file-downloads",
		"tagged-events",
		"form-submissions",
		"titles"
	];
	const file = $derived(
		["script", ...order.filter((feature) => features.includes(feature)), "js"].join(".")
	);
	const code = $derived(
		`<script defer data-domain="${domain}" src="${location.origin}/js/${file}"></` + `script>`
	);
</script>

<div class="flex flex-col gap-3">
	<Code {code} />
	<Checks
		bind:value={features}
		options={[
			{ key: "outbound-links", label: "outbound links" },
			{ key: "file-downloads", label: "downloads" },
			{ key: "tagged-events", label: "tagged events" },
			{ key: "form-submissions", label: "forms" },
			{ key: "hash", label: "hash routing" },
			{ key: "titles", label: "page titles" }
		]}
	/>
</div>
