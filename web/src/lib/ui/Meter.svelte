<script lang="ts">
	let { value, label }: { value: number; label: string } = $props();

	const percent = $derived(Math.round(Math.min(1, Math.max(0, value)) * 100));
</script>

<div class="flex items-center gap-3">
	<div
		role="progressbar"
		aria-label={label}
		aria-valuenow={percent}
		aria-valuemin={0}
		aria-valuemax={100}
		class="h-1.5 flex-1 overflow-hidden rounded-px bg-raised"
	>
		<div
			class="h-full bg-cyan-deep transition-[width] duration-300 ease-[steps(4)] motion-reduce:transition-none"
			style:width="{percent}%"
		></div>
	</div>
	<span class="w-10 text-right font-mono text-label text-dim">{percent}%</span>
</div>
