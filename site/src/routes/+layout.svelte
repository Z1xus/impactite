<script lang="ts">
	import "./layout.css";

	let { children } = $props();
</script>

<svelte:head>
	<title>impactite</title>
	<meta
		name="description"
		content="Self-hosted web analytics in one binary, compatible with Plausible and GoatCounter."
	/>
</svelte:head>

{@render children()}
