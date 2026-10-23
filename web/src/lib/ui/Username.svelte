<script lang="ts">
	let { value }: { value: string } = $props();
</script>

<!-- tells a password manager whose password the form saves -->
<input
	type="text"
	class="sr-only"
	autocomplete="username"
	tabindex="-1"
	aria-hidden="true"
	readonly
	{value}
/>
