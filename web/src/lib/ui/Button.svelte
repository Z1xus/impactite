<script lang="ts">
	import type { Snippet } from "svelte";
	import type { HTMLButtonAttributes } from "svelte/elements";

	let {
		children,
		variant = "primary",
		size = "md",
		busy = false,
		href,
		class: className = "",
		...rest
	}: HTMLButtonAttributes & {
		children: Snippet;
		variant?: "primary" | "quiet" | "danger" | "icon" | "tool" | "plain";
		size?: "md" | "sm";
		busy?: boolean;
		href?: string;
	} = $props();

	const variants = {
		primary:
			"border border-cyan bg-cyan text-ink enabled:not-focus-visible:hover:border-fg enabled:not-focus-visible:hover:bg-fg disabled:border-edge disabled:bg-raised disabled:text-faint disabled:opacity-100",
		quiet: "border border-edge text-fg enabled:hover:border-dim",
		danger: "border border-bad/50 text-bad enabled:hover:border-bad enabled:hover:bg-bad/10",
		icon: "size-control border border-edge text-dim enabled:hover:border-dim enabled:hover:text-fg",
		tool: "size-compact text-dim enabled:hover:text-fg",
		plain: "text-dim enabled:hover:text-fg"
	};
	const padding = { primary: "px-4", quiet: "px-3", danger: "px-3", plain: "px-2" };
	let pending = $state(false);
	const waiting = $derived(busy || pending);

	async function click(event: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }) {
		if (waiting) return event.preventDefault();
		const result: unknown = rest.onclick?.(event);
		if (!(result instanceof Promise)) return;
		pending = true;
		try {
			await result;
		} finally {
			pending = false;
		}
	}

	const shape = $derived(
		variant === "icon" || variant === "tool"
			? "text-body"
			: size === "sm"
				? "h-compact px-2.5 text-small"
				: `h-control text-body ${padding[variant]}`
	);
	const look = $derived(
		`inline-flex items-center justify-center gap-2 rounded-px font-sans ${variants[variant]} ${shape} ${waiting ? "cursor-wait opacity-60" : ""} ${className}`
	);
</script>

{#if href}
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- callers pass resolved paths -->
	<a {href} class={look}>{@render children()}</a>
{:else}
	<button {...rest} onclick={click} disabled={rest.disabled} aria-busy={waiting} class={look}>
		{@render children()}
	</button>
{/if}
