<script lang="ts">
	import { css, threshold } from "#lib/dither/bayer.ts";
	import { theme } from "#lib/theme.svelte.ts";
	import { elapsed } from "./format.ts";

	let {
		activity,
		pages,
		clicks,
		total,
		time,
		seek
	}: {
		// event times from the start, in ms
		activity: number[];
		pages: number[];
		clicks: number[];
		total: number;
		time: number;
		seek: (ms: number) => void;
	} = $props();

	const HEIGHT = 40;
	const CELL = 2;

	let canvas: HTMLCanvasElement;
	let width = $state(0);
	let hover = $state<number | null>(null);

	const at = (x: number) => Math.max(0, Math.min(total, (x / Math.max(1, width)) * total));

	$effect(() => {
		const ctx = canvas.getContext("2d");
		void theme.shown;
		if (!ctx || !width || !total) return;
		const columns = Math.floor(width / CELL);
		const rows = HEIGHT / CELL;
		canvas.width = columns;
		canvas.height = rows;
		const counts = new Array<number>(columns).fill(0);
		for (const ms of activity)
			counts[Math.min(columns - 1, Math.floor((ms / total) * columns))]++;
		const peak = Math.max(1, ...counts);
		const played = Math.floor((time / total) * columns);
		const [cyan, deep, night, fg, line] = [
			"--color-cyan",
			"--color-cyan-deep",
			"--color-cyan-night",
			"--color-fg",
			"--color-line"
		].map(css);
		ctx.clearRect(0, 0, columns, rows);
		const dot = (x: number, y: number, color: string) => {
			ctx.fillStyle = color;
			ctx.fillRect(x, y, 1, 1);
		};
		for (let x = 0; x < columns; x++) {
			const level = Math.ceil(Math.sqrt(counts[x] / peak) * (rows - 4));
			dot(x, rows - 1, line);
			for (let y = rows - 1; y >= rows - level; y--) {
				const depth = (rows - y) / rows;
				if (theme.style !== "plain" && threshold(x, y) > 0.85 - depth * 0.3) continue;
				dot(x, y, x <= played ? cyan : deep);
			}
			if (x > played && counts[x] === 0) dot(x, rows - 1, night);
		}
		const column = (ms: number) => Math.min(columns - 1, Math.floor((ms / total) * columns));
		for (const ms of pages) for (let y = 0; y < rows; y += 2) dot(column(ms), y, fg);
		for (const ms of clicks) {
			const x = column(ms);
			for (const [dx, dy] of [
				[0, 1],
				[1, 1],
				[0, 2],
				[1, 2]
			])
				dot(x + dx, dy, cyan);
		}
		for (let y = 0; y < rows; y++) dot(played, y, fg);
	});

	function pick(event: PointerEvent) {
		const box = canvas.getBoundingClientRect();
		seek(at(event.clientX - box.left));
	}
</script>

<div class="relative flex flex-col gap-2">
	<canvas
		bind:this={canvas}
		bind:clientWidth={width}
		class="h-10 w-full cursor-pointer pixelated"
		role="slider"
		tabindex="0"
		aria-label="time"
		aria-valuemin={0}
		aria-valuemax={Math.round(total / 1000)}
		aria-valuenow={Math.round(time / 1000)}
		aria-valuetext={elapsed(time)}
		onpointerdown={(event) => {
			canvas.setPointerCapture(event.pointerId);
			pick(event);
		}}
		onpointermove={(event) => {
			hover = event.offsetX;
			if (canvas.hasPointerCapture(event.pointerId)) pick(event);
		}}
		onpointerleave={() => (hover = null)}
		onkeydown={(event) => {
			const step = { ArrowRight: 5000, ArrowLeft: -5000 }[event.key];
			if (!step) return;
			event.preventDefault();
			seek(Math.max(0, Math.min(total, time + step)));
		}}
	></canvas>
	{#if hover !== null}
		<span
			class="pointer-events-none absolute -top-7 -translate-x-1/2 rounded-px border border-edge bg-raised px-1.5 py-1 num text-label text-fg"
			style:left="{hover}px">{elapsed(at(hover))}</span
		>
	{/if}
</div>
