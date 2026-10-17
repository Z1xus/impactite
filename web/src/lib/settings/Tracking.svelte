<script lang="ts">
	import Snippet from "./Snippet.svelte";
	import Section from "#lib/ui/Section.svelte";

	let { domain }: { domain: string } = $props();
</script>

<Section id="tracking" label="tracking">
	<Snippet {domain} />
</Section>
