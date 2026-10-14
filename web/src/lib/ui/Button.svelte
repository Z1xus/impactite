<script lang="ts">
	import type { Snippet } from "svelte";
	import type { HTMLButtonAttributes } from "svelte/elements";

	let {
		children,
		quiet = false,
		busy = false,
		class: className = "",
		...rest
	}: HTMLButtonAttributes & { children: Snippet; quiet?: boolean; busy?: boolean } = $props();
</script>

<button
	{...rest}
	disabled={busy || rest.disabled}
	class="inline-flex h-9 items-center justify-center gap-2 rounded-px border px-4 label transition-colors duration-100 enabled:cursor-pointer disabled:cursor-default {quiet
		? 'border-edge text-fg hover:border-dim'
		: 'border-cyan bg-cyan text-ink hover:border-fg hover:bg-fg'} {busy
		? 'cursor-wait opacity-60'
		: ''} {className}"
>
	{@render children()}
</button>
