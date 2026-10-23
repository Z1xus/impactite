<script lang="ts">
	import { css } from "#lib/dither/bayer.ts";
	import { COLORS, fit, GAP, HEIGHT, QUIET, smooth, strip, WIDTH } from "#lib/dither/strip.ts";
	import { theme } from "#lib/theme.svelte.ts";
	import { day, percent } from "#lib/uptime/format.ts";
	import type { Cell } from "#lib/uptime/types.ts";

	let {
		cells,
		today,
		paused = false,
		stretch = false,
		fitted
	}: {
		cells: Cell[];
		today: number;
		paused?: boolean;
		stretch?: boolean;
		fitted?: (days: number) => void;
	} = $props();

	let canvas = $state<HTMLCanvasElement>();
	let box: HTMLDivElement;
	let width = $state(0);
	let ratio = $state(1);
	let hover = $state<number | null>(null);

	const plain = $derived(theme.style === "plain");
	const unit = $derived(plain ? 1 : Math.max(1, Math.round(2 * ratio)));
	const room = $derived(Math.floor((width * ratio) / unit));
	const gap = $derived(plain ? Math.round(2 * ratio) : GAP);
	const layout = $derived(
		fit(room, gap, plain ? Math.round(4 * ratio) : WIDTH, cells.length, stretch)
	);
	const count = $derived(layout.count);

	$effect(() => {
		fitted?.(count);
	});
	const shown = $derived(cells.slice(cells.length - count));
	const tall = $derived(plain ? 22 : (HEIGHT * unit) / ratio);
	const span = $derived((layout.total * unit) / ratio);
	const shift = $derived(
		stretch ? Math.round(((width - span) * ratio) / 2) / ratio : Math.max(0, width - span)
	);

	function draw() {
		const ctx = canvas?.getContext("2d");
		if (!ctx || !width) return;
		const { wide, total } = layout;
		if (plain) {
			canvas!.width = total;
			canvas!.height = Math.round(tall * ratio);
			smooth(ctx, shown, paused, hover, [wide, gap, canvas!.height, ratio]);
			return;
		}
		canvas!.width = total * unit;
		canvas!.height = HEIGHT * unit;
		const palette = COLORS.map(css);
		const pixels = strip(shown, paused, hover, wide);
		for (let i = 0; i < pixels.length;) {
			const color = pixels[i];
			let run = 1;
			while ((i + run) % total && pixels[i + run] === color) run++;
			if (color) {
				ctx.globalAlpha = color === QUIET[0] ? 0.3 : color === QUIET[1] ? 0.55 : 1;
				ctx.fillStyle = palette[color - 1];
				ctx.fillRect((i % total) * unit, Math.floor(i / total) * unit, run * unit, unit);
			}
			i += run;
		}
	}

	$effect(() => {
		void [shown, paused, hover, layout, tall, theme.shown, plain];
		draw();
	});

	$effect(() => {
		const measure = () => {
			width = box.getBoundingClientRect().width;
			ratio = devicePixelRatio;
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(box);
		return () => observer.disconnect();
	});

	function move(event: PointerEvent) {
		const rect = canvas!.getBoundingClientRect();
		const at = Math.floor(((event.clientX - rect.left) / rect.width) * count);
		hover = at < 0 || at >= count ? null : at;
	}

	const cell = $derived(hover === null ? null : shown[hover]);
	const date = $derived(
		hover === null ? 0 : today - (cells.length - 1 - (cells.length - count + hover)) * 86_400
	);
	const tip = $derived(
		hover === null
			? 0
			: Math.min(
					Math.max(shift + ((hover + 0.5) / count) * span, 56),
					Math.max(width - 56, 56)
				)
	);
</script>

<svelte:window
	onpointerdown={(event) =>
		hover !== null && !box.contains(event.target as Node) && (hover = null)}
/>

<div
	bind:this={box}
	class="relative h-[22px] w-full min-w-0 touch-pan-y {stretch ? '' : 'max-w-[540px]'}"
	onpointerdown={move}
	onpointermove={move}
	onpointerleave={(event) => event.pointerType === "mouse" && (hover = null)}
	role="img"
	aria-label="uptime, last {count} days"
>
	<canvas
		bind:this={canvas}
		class="absolute bottom-0 pixelated"
		style:left="{shift}px"
		style:width="{span}px"
		style:height="{tall}px"
		aria-hidden="true"
	></canvas>
	{#if hover !== null}
		<div
			class="pointer-events-none absolute bottom-full z-20 mb-2 flex -translate-x-1/2 flex-col gap-1.5 rounded-px border border-edge bg-raised px-2.5 py-2 whitespace-nowrap"
			style:left="{tip}px"
		>
			<span class="label">{day(date)}</span>
			{#if cell}
				<span class="num text-fg">{percent(cell[0])}</span>
				<span class="num text-label text-faint">
					{cell[2]
						? `${cell[2]} ${cell[2] === 1 ? "incident" : "incidents"}`
						: "no incidents"}
				</span>
			{:else}
				<span class="num text-label text-faint">no data</span>
			{/if}
		</div>
	{/if}
</div>
