<script lang="ts">
	import { access } from "#lib/api.ts";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Icon from "./Icon.svelte";

	let {
		spec,
		fallback,
		eager = false
	}: { spec: Spec | null; fallback?: string; eager?: boolean } = $props();

	let broken = $state(false);
	let loaded = $state(false);

	$effect(() => {
		void spec;
		broken = false;
		loaded = false;
	});

	const color = $derived.by(() => {
		if (spec?.kind !== "brand") return null;
		const n = parseInt(spec.hex, 16);
		const luma = (((n >> 16) & 255) * 0.3 + ((n >> 8) & 255) * 0.59 + (n & 255) * 0.11) / 255;
		return luma < 0.22 ? "var(--color-fg)" : `#${spec.hex}`;
	});
	const missing = $derived(!spec || (spec.kind === "image" && broken));
	const letter = $derived(fallback?.match(/[a-z0-9]/i)?.[0]?.toUpperCase());
</script>

{#if !missing || letter}
	<span
		class="relative grid h-5 w-5 shrink-0 place-items-center rounded-[3px] bg-raised"
		aria-hidden="true"
	>
		<span
			class="absolute inset-0 rounded-[3px] border {color ? 'opacity-55' : 'border-edge'}"
			style:border-color={color}
		></span>
		{#if missing}
			<span class="font-mono text-label leading-none text-dim">{letter}</span>
		{:else if spec?.kind === "brand"}
			<svg width="12" height="12" viewBox="0 0 24 24" fill={color}>
				<path d={spec.path} />
			</svg>
		{:else if spec?.kind === "image"}
			{#if eager && !loaded && letter}
				<span class="absolute font-mono text-label leading-none text-dim">{letter}</span>
			{/if}
			<img
				src={access.share && spec.url.startsWith("/api/")
					? `${spec.url}?share=${encodeURIComponent(access.share)}`
					: spec.url}
				alt=""
				loading={eager ? "eager" : "lazy"}
				onload={() => (loaded = true)}
				class={spec.flag ? "h-[9px] w-3 rounded-[1px] object-cover" : "h-3 w-3"}
				onerror={() => (broken = true)}
			/>
		{:else if spec?.kind === "pixel"}
			<Icon svg={spec.svg} size={12} class="text-dim" />
		{/if}
	</span>
{:else}
	<span class="h-5 w-5 shrink-0" aria-hidden="true"></span>
{/if}
