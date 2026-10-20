<script lang="ts">
	import { theme } from "#lib/theme.svelte.ts";

	let { size = 22 }: { size?: number } = $props();
</script>

{#if theme.style === "plain"}
	<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
		<path
			fill="var(--color-cyan-night)"
			d="M0 52q10-1 14-9l4 9q14 16 28 0l4-9q4 8 14 9v12H0z"
		/>
		<g fill="var(--color-cyan-deep)">
			<circle cx="6" cy="34" r="2" />
			<circle cx="10" cy="26" r="2" />
			<circle cx="58" cy="34" r="2" />
			<circle cx="62" cy="26" r="2" />
		</g>
		<rect
			fill="color-mix(in srgb, var(--color-cyan) 40%, var(--color-cyan-deep))"
			x="16"
			y="40"
			width="8"
			height="14"
			rx="2"
		/>
		<rect
			fill="color-mix(in srgb, var(--color-cyan) 65%, var(--color-cyan-deep))"
			x="28"
			y="28"
			width="8"
			height="32"
			rx="2"
		/>
		<rect fill="var(--color-cyan)" x="40" y="16" width="8" height="38" rx="2" />
		<path stroke="var(--color-cyan)" stroke-width="3" stroke-linecap="round" d="M54 10l6-6" />
		<circle fill="var(--color-fg)" cx="54" cy="10" r="2.5" />
	</svg>
{:else}
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		shape-rendering="crispEdges"
		aria-hidden="true"
	>
		<path
			fill="color-mix(in srgb, var(--color-cyan-deep) 40%, var(--color-cyan-night))"
			d="M0 48h4v4h-4zM4 48h4v4h-4zM8 44h4v4h-4zM12 40h4v4h-4zM20 52h4v4h-4zM24 56h4v4h-4zM28 60h4v4h-4zM32 60h4v4h-4zM36 60h4v4h-4zM40 56h4v4h-4zM48 44h4v4h-4zM52 40h4v4h-4zM56 44h4v4h-4zM60 48h4v4h-4z"
		/>
		<path
			fill="var(--color-cyan-night)"
			d="M0 52h4v12h-4zM4 52h4v12h-4zM8 48h4v16h-4zM12 44h4v20h-4zM16 52h4v12h-4zM20 56h4v8h-4zM24 60h4v4h-4zM40 60h4v4h-4zM44 56h4v8h-4zM48 48h4v16h-4zM52 44h4v20h-4zM56 48h4v16h-4zM60 52h4v12h-4z"
		/>
		<path
			fill="var(--color-cyan-deep)"
			d="M4 32h4v4h-4zM8 24h4v4h-4zM56 32h4v4h-4zM60 24h4v4h-4z"
		/>
		<path
			fill="color-mix(in srgb, var(--color-cyan) 40%, var(--color-cyan-deep))"
			d="M16 40h4v12h-4zM20 40h4v12h-4z"
		/>
		<path
			fill="color-mix(in srgb, var(--color-cyan) 65%, var(--color-cyan-deep))"
			d="M28 28h4v32h-4zM32 28h4v32h-4z"
		/>
		<path fill="var(--color-cyan)" d="M40 16h4v40h-4zM44 16h4v40h-4zM56 4h4v4h-4z" />
		<path fill="var(--color-fg)" d="M52 8h4v4h-4z" />
	</svg>
{/if}
