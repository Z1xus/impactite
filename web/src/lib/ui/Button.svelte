<script lang="ts">
	import type { Snippet } from "svelte";
	import type { HTMLButtonAttributes } from "svelte/elements";

	let {
		children,
		variant = "primary",
		busy = false,
		class: className = "",
		...rest
	}: HTMLButtonAttributes & {
		children: Snippet;
		variant?: "primary" | "quiet" | "plain";
		busy?: boolean;
	} = $props();

	const variants = {
		primary: "h-9 border border-cyan bg-cyan px-4 text-ink hover:border-fg hover:bg-fg",
		quiet: "h-8 border border-edge px-3 text-fg hover:border-dim",
		plain: "h-7 px-1 text-dim hover:text-fg"
	};
</script>

<button
	{...rest}
	disabled={busy || rest.disabled}
	class="inline-flex items-center justify-center gap-2 rounded-px label transition-colors duration-100 enabled:cursor-pointer disabled:cursor-default {variants[
		variant
	]} {busy ? 'cursor-wait opacity-60' : ''} {className}"
>
	{@render children()}
</button>
