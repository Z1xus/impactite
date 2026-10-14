<script lang="ts">
	import type { Spec } from "#lib/icons/resolve.ts";
	import Icon from "./Icon.svelte";

	let { spec, fallback }: { spec: Spec | null; fallback?: string } = $props();

	let broken = $state(false);

	$effect(() => {
		void spec;
		broken = false;
	});

	function visible(hex: string) {
		const n = parseInt(hex, 16);
		const luma = (((n >> 16) & 255) * 0.3 + ((n >> 8) & 255) * 0.59 + (n & 255) * 0.11) / 255;
		return luma < 0.22 ? "var(--color-fg)" : `#${hex}`;
	}
</script>

<span class="grid h-4 w-4 shrink-0 place-items-center" aria-hidden="true">
	{#if spec?.kind === "brand"}
		<svg width="14" height="14" viewBox="0 0 24 24" fill={visible(spec.hex)}>
			<path d={spec.path} />
		</svg>
	{:else if spec?.kind === "image" && !broken}
		<img
			src={spec.url}
			alt=""
			loading="lazy"
			class={spec.flag ? "h-3 w-4 rounded-[1px] object-cover" : "h-4 w-4 rounded-[2px]"}
			onerror={() => (broken = true)}
		/>
	{:else if spec?.kind === "pixel"}
		<Icon svg={spec.svg} size={16} class="text-dim" />
	{:else if fallback}
		<span
			class="grid h-4 w-4 place-items-center rounded-[2px] bg-line font-mono text-[11px] leading-none text-dim"
		>
			{fallback.slice(0, 1).toUpperCase()}
		</span>
	{/if}
</span>
