<script lang="ts">
	import { untrack } from "svelte";
	import { theme } from "#lib/theme.svelte.ts";
	import { css, threshold } from "./bayer";

	let {
		values,
		previous = [],
		labels = [],
		keys = labels,
		format = (value: number) => String(value),
		height = 220,
		cell = 2,
		axes = true
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
	} = $props();

	let canvas: HTMLCanvasElement;
	let box: HTMLDivElement;
	let width = $state(0);
	let hover = $state<number | null>(null);

	let shown: number[] = [];
	let shownPrevious: number[] = [];
	let from: number[] = [];
	let fromPrevious: number[] = [];
	let started = 0;
	let frame = 0;

	const MORPH = 280;
	const SLIDE = 420;
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

	function sample(series: number[], x: number, columns: number) {
		if (series.length < 2) return series[0] ?? 0;
		const at = (x / (columns - 1)) * (series.length - 1);
		const low = Math.floor(at);
		const high = Math.min(series.length - 1, low + 1);
		return series[low] + (series[high] - series[low]) * (at - low);
	}

	function draw() {
		const ctx = canvas?.getContext("2d");
		if (!ctx || !width) return;
		const columns = Math.floor(width / cell);
		const rows = Math.floor(height / cell);
		canvas.width = columns;
		canvas.height = rows;
		const image = ctx.createImageData(columns, rows);
		const data = image.data;
		const colors = {
			line: rgb(css("--color-cyan")),
			deep: rgb(css("--color-cyan-deep")),
			night: rgb(css("--color-cyan-night")),
			grid: rgb(css("--color-line")),
			faint: rgb(css("--color-faint")),
			fg: rgb(css("--color-fg"))
		};
		const put = (x: number, y: number, [r, g, b]: number[]) => {
			if (x < 0 || y < 0 || x >= columns || y >= rows) return;
			const i = (y * columns + x) * 4;
			data[i] = r;
			data[i + 1] = g;
			data[i + 2] = b;
			data[i + 3] = 255;
		};
		const top = (value: number) => Math.round((1 - value / peak) * (rows - 1));

		if (axes) {
			for (const tick of ticks) {
				const y = top(tick);
				for (let x = 0; x < columns; x += 2) put(x, y, colors.grid);
			}
		}

		let lastY = top(at(0, columns));
		for (let x = 0; x < columns; x++) {
			const y = top(at(x, columns));
			for (let yy = y + 1; yy < rows; yy++) {
				const depth = (yy - y) / Math.max(1, rows - y);
				const density = 0.3 * (1 - depth) ** 1.6;
				if (threshold(x, yy) < density)
					put(x, yy, depth < 0.18 ? colors.deep : colors.night);
			}
			for (let yy = Math.min(y, lastY); yy <= Math.max(y, lastY); yy++)
				put(x, yy, colors.line);
			lastY = y;
		}

		if (shownPrevious.length) {
			for (let x = 0; x < columns; x += 2)
				put(x, top(sample(shownPrevious, x, columns)), colors.faint);
		}

		if (hover !== null && shown.length) {
			const x = Math.round((hover / Math.max(1, shown.length - 1)) * (columns - 1));
			for (let y = 0; y < rows; y += 2) put(x, y, colors.faint);
			const y = top(shown[hover] ?? 0);
			for (let dx = -1; dx <= 1; dx++)
				for (let dy = -1; dy <= 1; dy++) put(x + dx, y + dy, colors.fg);
		}

		ctx.putImageData(image, 0, 0);
	}

	function rgb(hex: string) {
		const n = parseInt(hex.slice(1), 16);
		return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
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
		from = resample(shown, next.length);
		fromPrevious = resample(shownPrevious, nextPrevious.length);
		cancelAnimationFrame(frame);
		if (still) {
			shown = next;
			shownPrevious = nextPrevious;
			untrack(draw);
			return;
		}
		started = performance.now();
		frame = requestAnimationFrame(animate);
		return () => cancelAnimationFrame(frame);
	});

	$effect(() => {
		void width;
		void hover;
		void theme.shown;
		draw();
	});

	$effect(() => {
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

	const tipLeft = $derived(hover === null ? 0 : (hover / Math.max(1, values.length - 1)) * width);
</script>

<div class="flex gap-3">
	{#if axes}
		<div class="relative min-w-11" style:height="{height}px">
			{#each ticks as tick, i (tick)}
				<span
					class="absolute right-0 label {i === 1
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
			class="relative"
			style:height="{height}px"
			onpointermove={move}
			onpointerleave={() => (hover = null)}
			role="img"
			aria-label="chart"
		>
			<canvas
				bind:this={canvas}
				class="absolute inset-0 h-full w-full pixelated"
				aria-hidden="true"
			></canvas>
			{#if hover !== null}
				<div
					class="pointer-events-none absolute top-0 z-10 flex flex-col gap-1.5 rounded-px border border-edge bg-raised px-2.5 py-2 whitespace-nowrap transition-[left] duration-75 ease-out {tipLeft >
					width / 2
						? '-translate-x-[calc(100%+12px)]'
						: 'translate-x-3'}"
					style:left="{tipLeft}px"
				>
					<span class="label">{labels[hover] ?? ""}</span>
					<span class="num text-fg">{format(values[hover] ?? 0)}</span>
					{#if previous.length}
						<span class="num text-label text-faint">{format(previous[hover] ?? 0)}</span
						>
					{/if}
				</div>
			{/if}
		</div>
		{#if axes && labels.length}
			<div class="mt-2 flex justify-between">
				<span class="label">{labels[0]}</span>
				<span class="label">{labels[Math.floor((labels.length - 1) / 2)]}</span>
				<span class="label">{labels[labels.length - 1]}</span>
			</div>
		{/if}
	</div>
</div>
