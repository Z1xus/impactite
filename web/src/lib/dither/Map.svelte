<script lang="ts">
	import { COLS, CODES, GRID, ROWS } from "#lib/art/world.ts";
	import { theme } from "#lib/theme.svelte.ts";
	import { css, threshold } from "./bayer";

	let {
		values,
		label,
		format,
		pick
	}: {
		values: Record<string, number>;
		label: (code: string) => string;
		format: (value: number) => string;
		pick: (code: string) => void;
	} = $props();

	const CELL = 4;
	let canvas: HTMLCanvasElement;
	let hover = $state<{ code: string; x: number; y: number } | null>(null);

	const code = (col: number, row: number) => {
		const char = GRID.charCodeAt(row * COLS + col);
		return char === 32 || Number.isNaN(char) ? "" : CODES[char - 48];
	};

	$effect(() => {
		const ctx = canvas.getContext("2d");
		if (!ctx) return;
		void theme.shown;
		const max = Math.max(1, ...Object.values(values));
		const [land, lit, active] = [css("--color-edge"), css("--color-cyan"), css("--color-fg")];
		const image = ctx.createImageData(COLS * CELL, ROWS * CELL);
		const paint = (x: number, y: number, color: string) => {
			const n = parseInt(color.slice(1), 16);
			const i = (y * COLS * CELL + x) * 4;
			image.data.set([n >> 16, (n >> 8) & 255, n & 255, 255], i);
		};
		for (let row = 0; row < ROWS; row++) {
			for (let col = 0; col < COLS; col++) {
				const country = code(col, row);
				if (!country) continue;
				const visits = values[country] ?? 0;
				const level = visits ? 0.2 + (0.8 * Math.log1p(visits)) / Math.log1p(max) : 0;
				for (let dy = 0; dy < CELL - 1; dy++) {
					for (let dx = 0; dx < CELL - 1; dx++) {
						const x = col * CELL + dx;
						const y = row * CELL + dy;
						const color =
							hover?.code === country ? active : threshold(x, y) < level ? lit : land;
						paint(x, y, color);
					}
				}
			}
		}
		ctx.putImageData(image, 0, 0);
	});

	function locate(event: PointerEvent) {
		const box = canvas.getBoundingClientRect();
		const col = Math.floor(((event.clientX - box.left) / box.width) * COLS);
		const row = Math.floor(((event.clientY - box.top) / box.height) * ROWS);
		const country = code(col, row);
		hover = country
			? { code: country, x: event.clientX - box.left, y: event.clientY - box.top }
			: null;
	}
</script>

<div class="relative">
	<canvas
		bind:this={canvas}
		width={COLS * CELL}
		height={ROWS * CELL}
		class="block w-full pixelated {hover ? 'cursor-pointer' : ''}"
		aria-label="visitors by country"
		onpointermove={locate}
		onpointerleave={() => (hover = null)}
		onclick={() => hover && pick(hover.code)}
	></canvas>
	{#if hover}
		<div
			class="pointer-events-none absolute z-10 flex -translate-x-1/2 -translate-y-full items-center gap-3 rounded-px border border-edge bg-raised px-3 py-2 whitespace-nowrap"
			style:left="{hover.x}px"
			style:top="{hover.y - 8}px"
		>
			<span class="text-[13px] text-fg">{label(hover.code)}</span>
			<span class="label text-cyan">{format(values[hover.code] ?? 0)}</span>
		</div>
	{/if}
</div>
