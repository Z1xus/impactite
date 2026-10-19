<script lang="ts">
	import { page } from "$app/state";
	import Failed from "#lib/ui/Failed.svelte";
</script>

<svelte:head>
	<title>{page.status === 404 ? "Not found" : "Error"} · impactite</title>
</svelte:head>

<Failed
	text={page.status === 404 ? "This page doesn't exist." : "Something went wrong."}
	retry={page.url.pathname.startsWith("/share/") ? () => location.reload() : undefined}
/>
