<script lang="ts">
	import { resolve } from "$app/paths";

	let { text = "This site doesn't exist, or you don't have access to it." }: { text?: string } =
		$props();
</script>

<div class="flex flex-col items-center gap-5 py-24">
	<p class="text-[14px] text-dim">{text}</p>
	<a
		href={resolve("/")}
		class="inline-flex h-9 items-center rounded-px border border-edge px-3 text-[14px] text-fg transition-colors duration-100 hover:border-dim"
		>All sites</a
	>
</div>
