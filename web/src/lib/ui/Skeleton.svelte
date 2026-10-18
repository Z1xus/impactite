<script lang="ts">
	let { class: className = "" }: { class?: string } = $props();
</script>

<span class="block rounded-px bg-line {className}"></span>
