<script lang="ts">
	let { size = 22 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 64 64" shape-rendering="crispEdges" aria-hidden="true">
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
