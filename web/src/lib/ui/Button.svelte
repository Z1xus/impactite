<script lang="ts">
	import type { Snippet } from "svelte";
	import type { HTMLButtonAttributes } from "svelte/elements";

	let {
		children,
		variant = "primary",
		busy = false,
		href,
		class: className = "",
		...rest
	}: HTMLButtonAttributes & {
		children: Snippet;
		variant?: "primary" | "quiet" | "danger" | "icon" | "plain";
		busy?: boolean;
		href?: string;
	} = $props();

	const variants = {
		primary:
			"h-control border border-cyan bg-cyan px-4 text-ink enabled:hover:border-fg enabled:hover:bg-fg",
		quiet: "h-control border border-edge px-3 text-fg enabled:hover:border-dim",
		danger: "h-control border border-bad/50 px-3 text-bad enabled:hover:border-bad enabled:hover:bg-bad/10",
		icon: "size-control border border-edge text-dim enabled:hover:border-dim enabled:hover:text-fg",
		plain: "h-control px-2 text-dim enabled:hover:text-fg"
	};
	let pending = $state(false);

	async function click(event: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }) {
		const result: unknown = rest.onclick?.(event);
		if (!(result instanceof Promise)) return;
		pending = true;
		try {
			await result;
		} finally {
			pending = false;
		}
	}

	const look = $derived(
		`inline-flex items-center justify-center gap-2 rounded-px font-sans text-body ${variants[variant]} ${busy || pending ? "disabled:cursor-wait disabled:opacity-60" : ""} ${className}`
	);
</script>

{#if href}
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- callers pass resolved paths -->
	<a {href} class={look}>{@render children()}</a>
{:else}
	<button {...rest} onclick={click} disabled={busy || pending || rest.disabled} class={look}>
		{@render children()}
	</button>
{/if}
