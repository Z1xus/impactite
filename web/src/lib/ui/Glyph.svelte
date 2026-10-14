<script lang="ts">
	import type { Spec } from "#lib/icons/resolve.ts";
	import Icon from "./Icon.svelte";

	let { spec, fallback }: { spec: Spec | null; fallback?: string } = $props();

	let broken = $state(false);

	$effect(() => {
		void spec;
		broken = false;
	});

	const color = $derived.by(() => {
		if (spec?.kind !== "brand") return null;
		const n = parseInt(spec.hex, 16);
		const luma = (((n >> 16) & 255) * 0.3 + ((n >> 8) & 255) * 0.59 + (n & 255) * 0.11) / 255;
		return luma < 0.22 ? "var(--color-fg)" : `#${spec.hex}`;
	});
	const missing = $derived(!spec || (spec.kind === "image" && broken));
</script>

{#if !missing || fallback}
	<span
		class="relative grid h-5 w-5 shrink-0 place-items-center rounded-[3px] bg-raised"
		aria-hidden="true"
	>
		<span
			class="absolute inset-0 rounded-[3px] border {color ? 'opacity-55' : 'border-edge'}"
			style:border-color={color}
		></span>
		{#if missing}
			<span class="font-mono text-[11px] leading-none text-dim"
				>{fallback?.slice(0, 1).toUpperCase()}</span
			>
		{:else if spec?.kind === "brand"}
			<svg width="12" height="12" viewBox="0 0 24 24" fill={color}>
				<path d={spec.path} />
			</svg>
		{:else if spec?.kind === "image"}
			<img
				src={spec.url}
				alt=""
				loading="lazy"
				class={spec.flag ? "h-[9px] w-3 rounded-[1px] object-cover" : "h-3 w-3"}
				onerror={() => (broken = true)}
			/>
		{:else if spec?.kind === "pixel"}
			<Icon svg={spec.svg} size={12} class="text-dim" />
		{/if}
	</span>
{/if}
