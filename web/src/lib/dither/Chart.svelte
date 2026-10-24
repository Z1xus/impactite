<script lang="ts">
	import { untrack } from "svelte";
	import { theme } from "#lib/theme.svelte.ts";
	import { css } from "./bayer";
	import { chart, COLORS, sample, smooth, type Band, type Mark } from "./chart.ts";
	import { paint } from "./pixels.ts";
	import Pixels from "./Pixels.svelte";

	let {
		values,
		previous = [],
		labels = [],
		keys = labels,
		format = (value: number) => String(value),
		height = 220,
		cell = 2,
		axes = true,
		bands = [],
		marks = [],
		mark
	}: {
		values: number[];
		previous?: number[];
		labels?: string[];
		// what each point stands for, the full time, so a shift isn't faked by repeating labels
		keys?: string[];
		format?: (value: number) => string;
		height?: number;
		cell?: number;
		axes?: boolean;
		bands?: Band[];
		marks?: Mark[];
		// clicking a point with a mouse, like adding a note there
		mark?: { label: string; pick: (at: number) => void };
	} = $props();

	let canvas: HTMLCanvasElement;
	let box: HTMLDivElement;
	let width = $state(0);
	let ready = $state(false);
	let hover = $state<number | null>(null);
	let tipWidth = $state(0);

	let shown: number[] = [];
	let shownPrevious: number[] = [];
	let from: number[] = [];
	let fromPrevious: number[] = [];
	let started = 0;
	let frame = 0;

	const MORPH = 280;
	const SLIDE = 420;
	// server rendering uses a fixed grid until the container can be measured
	const INITIAL_COLUMNS = 480;
	// labels the drawn series had, and a scroll in progress when time moved on
	let shownLabels: string[] = [];
	let slide: { series: number[]; shift: number; offset: number } | null = null;
	const ease = (t: number) => 1 - (1 - t) ** 3;

	const peak = $derived(nice(Math.max(1, ...values, ...previous)));
	const ticks = $derived([peak, peak / 2, 0]);

	function nice(max: number) {
		const step = 10 ** Math.floor(Math.log10(max));
		// counts get a whole middle tick, or 7.5 would read as 8
		const whole = [...values, ...previous].every(Number.isInteger);
		return (
			[1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10]
				.map((m) => m * step)
				.find((n) => n >= max && (!whole || Number.isInteger(n / 2))) ?? max
		);
	}

	// how many points the new series moved along in time, 0 when it changed another way
	function shiftOf(next: string[]) {
		const k = shownLabels.indexOf(next[0]);
		if (k <= 0 || next.length !== shownLabels.length || k > next.length / 2) return 0;
		return next.slice(0, next.length - k).every((label, i) => label === shownLabels[i + k])
			? k
			: 0;
	}

	// the value drawn at a column, read off the scrolling series while it slides
	function at(x: number, columns: number) {
		if (!slide) return sample(shown, x, columns);
		const n = shown.length;
		const position = slide.offset + (columns === 1 ? 0 : (x / (columns - 1)) * (n - 1));
		const low = Math.floor(position);
		const high = Math.min(slide.series.length - 1, low + 1);
		return slide.series[low] + (slide.series[high] - slide.series[low]) * (position - low);
	}

	function resample(source: number[], length: number) {
		if (!source.length) return new Array(length).fill(0);
		if (source.length === length) return source;
		return Array.from({ length }, (_, i) => {
			const at = length === 1 ? 0 : (i / (length - 1)) * (source.length - 1);
			const low = Math.floor(at);
			const high = Math.min(source.length - 1, low + 1);
			return source[low] + (source[high] - source[low]) * (at - low);
		});
	}

	function draw() {
		const ctx = canvas?.getContext("2d");
		if (!ctx || !width) return;
		if (theme.style === "plain") {
			canvas.width = Math.round(width * devicePixelRatio);
			canvas.height = Math.round(height * devicePixelRatio);
			ctx.scale(devicePixelRatio, devicePixelRatio);
			smooth(ctx, width, height, peak, axes, shown, shownPrevious, hover, at, bands, marks);
			ready = true;
			return;
		}
		const columns = Math.max(1, Math.floor(width / cell));
		const rows = Math.floor(height / cell);
		canvas.width = columns;
		canvas.height = rows;
		const palette = COLORS.map((name) => {
			const n = parseInt(css(name).slice(1), 16);
			return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
		});
		paint(
			ctx,
			chart(columns, rows, peak, axes, shown, shownPrevious, hover, at, bands, marks),
			columns,
			palette
		);
		ready = true;
	}

	function animate(now: number) {
		const t = Math.min(1, (now - started) / MORPH);
		const k = ease(t);
		shown = values.map((v, i) => from[i] + (v - from[i]) * k);
		shownPrevious = previous.map((v, i) => fromPrevious[i] + (v - fromPrevious[i]) * k);
		draw();
		if (t < 1) frame = requestAnimationFrame(animate);
	}

	function glide(now: number) {
		if (!slide) return;
		const t = Math.min(1, (now - started) / SLIDE);
		slide.offset = slide.shift * ease(t);
		if (t >= 1) slide = null;
		draw();
		if (slide) frame = requestAnimationFrame(glide);
	}

	$effect(() => {
		const next = values;
		const nextPrevious = previous;
		const nextLabels = keys;
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		const shift = untrack(() => shiftOf(nextLabels));
		shownLabels = nextLabels;
		cancelAnimationFrame(frame);
		// time moved on, so the line scrolls instead of every point changing height
		if (shift && !still) {
			slide = { series: [...shown.slice(0, shift), ...next], shift, offset: 0 };
			shown = next;
			shownPrevious = nextPrevious;
			started = performance.now();
			frame = requestAnimationFrame(glide);
			return () => cancelAnimationFrame(frame);
		}
		slide = null;
		if (still || !shown.length) {
			shown = next;
			shownPrevious = nextPrevious;
			untrack(draw);
			return;
		}
		from = resample(shown, next.length);
		fromPrevious = resample(shownPrevious, nextPrevious.length);
		started = performance.now();
		frame = requestAnimationFrame(animate);
		return () => cancelAnimationFrame(frame);
	});

	$effect(() => {
		void width;
		void hover;
		void bands;
		void marks;
		void theme.shown;
		void theme.style;
		draw();
	});

	$effect(() => {
		width = box.getBoundingClientRect().width;
		const observer = new ResizeObserver(([entry]) => (width = entry.contentRect.width));
		observer.observe(box);
		return () => observer.disconnect();
	});

	function move(event: PointerEvent) {
		if (!values.length) return;
		const rect = box.getBoundingClientRect();
		const at = (event.clientX - rect.left) / rect.width;
		hover = Math.max(0, Math.min(values.length - 1, Math.round(at * (values.length - 1))));
	}

	const banded = $derived(
		bands.find((band) => (hover ?? -1) >= band.from && (hover ?? -1) <= band.to)
	);
	const marked = $derived(marks.filter((item) => item.at === hover));
	const tipLeft = $derived(hover === null ? 0 : (hover / Math.max(1, values.length - 1)) * width);
</script>

<svelte:window
	onpointerdown={(event) =>
		hover !== null && !box.contains(event.target as Node) && (hover = null)}
/>

<div class="flex gap-3">
	{#if axes}
		<div class="relative min-w-11" style:height="{height}px">
			{#each ticks as tick, i (tick)}
				<span
					class="absolute right-0 label whitespace-nowrap {i === 1
						? '-translate-y-1/2'
						: i === 2
							? '-translate-y-full'
							: ''}"
					style:top="{i * 50}%">{format(tick)}</span
				>
			{/each}
		</div>
	{/if}
	<div class="relative min-w-0 flex-1">
		<div
			bind:this={box}
			class="relative touch-pan-y"
			style:height="{height}px"
			onpointerdown={move}
			onpointermove={move}
			onpointerleave={(event) => event.pointerType === "mouse" && (hover = null)}
			onpointerup={(event) =>
				mark &&
				event.pointerType === "mouse" &&
				event.button === 0 &&
				hover !== null &&
				mark.pick(hover)}
			role="img"
			aria-label="chart"
		>
			{#if !ready}
				<Pixels
					pixels={chart(
						INITIAL_COLUMNS,
						Math.floor(height / cell),
						peak,
						axes,
						values,
						previous
					)}
					width={INITIAL_COLUMNS}
					palette={COLORS.map((name) => `var(${name})`)}
				/>
			{/if}
			<canvas
				bind:this={canvas}
				class="absolute inset-0 h-full w-full pixelated"
				aria-hidden="true"
			></canvas>
			{#if hover !== null}
				<div
					bind:clientWidth={tipWidth}
					class="pointer-events-none absolute top-0 z-10 flex flex-col gap-1.5 rounded-px border border-edge bg-raised px-2.5 py-2 whitespace-nowrap transition-[left] duration-75 ease-out {tipLeft >
						width / 2 || tipLeft + tipWidth + 12 > width
						? '-translate-x-[calc(100%+12px)]'
						: 'translate-x-3'}"
					style:left="{tipLeft}px"
				>
					<span class="label">{labels[hover] ?? ""}</span>
					<span class="num text-fg">{format(values[hover] ?? 0)}</span>
					{#if banded?.label}
						<span class="num text-label text-bad">{banded.label}</span>
						{#each banded.notes ?? [] as note (note)}
							<span class="num text-label text-dim">{note}</span>
						{/each}
					{/if}
					{#if previous.length}
						<span class="num text-label text-faint">{format(previous[hover] ?? 0)}</span
						>
					{/if}
					{#each marked as item, i (i)}
						{#each item.lines as line, j (j)}
							<span
								class="max-w-64 truncate text-small {item.tone === 'spike'
									? 'text-cyan'
									: j
										? 'text-dim'
										: 'text-fg'}">{line}</span
							>
						{/each}
					{/each}
					{#if mark}
						<span class="label text-faint pointer-coarse:hidden">{mark.label}</span>
					{/if}
				</div>
			{/if}
		</div>
		{#if axes && labels.length}
			<div class="mt-2 flex justify-between whitespace-nowrap">
				<span class="label">{labels[0]}</span>
				<span class="label {labels[0].length > 8 ? 'max-sm:hidden' : ''}"
					>{labels[Math.floor((labels.length - 1) / 2)]}</span
				>
				<span class="label">{labels[labels.length - 1]}</span>
			</div>
		{/if}
	</div>
</div>
