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
		variant?: "primary" | "quiet" | "danger" | "icon" | "plain" | "text";
		busy?: boolean;
	} = $props();

	const variants = {
		primary: "h-9 border border-cyan bg-cyan px-4 text-ink hover:border-fg hover:bg-fg",
		quiet: "h-9 border border-edge px-3 text-fg hover:border-dim",
		danger: "h-9 border border-bad/50 px-3 text-bad hover:border-bad hover:bg-bad/10",
		icon: "h-9 w-9 border border-edge text-dim hover:border-dim hover:text-fg",
		plain: "h-9 px-1 text-dim hover:text-fg",
		text: "h-9 px-2 font-sans text-[14px] tracking-normal normal-case text-dim hover:bg-raised hover:text-fg"
	};
</script>

<button
	{...rest}
	disabled={busy || rest.disabled}
	class="inline-flex items-center justify-center gap-2 rounded-px font-sans text-[14px] transition-colors duration-100 enabled:cursor-pointer disabled:cursor-default {variants[
		variant
	]} {busy ? 'cursor-wait opacity-60' : ''} {className}"
>
	{@render children()}
</button>
