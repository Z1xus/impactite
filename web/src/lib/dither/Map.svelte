<script lang="ts">
	import { COLS, CODES, GRID, ROWS } from "#lib/art/world.ts";
	import { theme } from "#lib/theme.svelte.ts";
	import { css } from "./bayer";

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
	let canvas = $state<HTMLCanvasElement>();
	let shapes = $state<Record<string, string>>();
	let hover = $state<{ code: string; x: number; y: number } | null>(null);

	const code = (col: number, row: number) => {
		const char = GRID.charCodeAt(row * COLS + col);
		return char === 32 || Number.isNaN(char) ? "" : CODES[char - 48];
	};

	const range = $derived.by(() => {
		const counts = Object.values(values).filter((value) => value > 0);
		return [Math.log(Math.min(...counts)), Math.log(Math.max(...counts))];
	});
	// four clear steps on a log scale between the least and most visited country
	const share = (country: string) => {
		const visits = values[country] ?? 0;
		if (!visits) return 0;
		const [low, high] = range;
		const at = high > low ? (Math.log(visits) - low) / (high - low) : 1;
		return (1 + Math.min(3, Math.floor(at * 4))) / 4;
	};

	$effect(() => {
		if (theme.style === "plain") import("#lib/art/shapes.ts").then((m) => (shapes = m.SHAPES));
	});

	$effect(() => {
		const ctx = canvas?.getContext("2d");
		if (!ctx) return;
		void theme.shown;
		const [land, cyan, active] = [css("--color-edge"), css("--color-cyan"), css("--color-fg")];
		const rgb = (hex: string) => [16, 8, 0].map((k) => (parseInt(hex.slice(1), 16) >> k) & 255);
		const [from, to] = [rgb(land), rgb(cyan)];
		const image = ctx.createImageData(COLS * CELL, ROWS * CELL);
		for (let row = 0; row < ROWS; row++) {
			for (let col = 0; col < COLS; col++) {
				const country = code(col, row);
				if (!country) continue;
				const lit = share(country);
				const color =
					hover?.code === country
						? rgb(active)
						: from.map((v, k) => v + (to[k] - v) * lit);
				for (let dy = 0; dy < CELL - 1; dy++)
					for (let dx = 0; dx < CELL - 1; dx++)
						image.data.set(
							[...color, 255],
							((row * CELL + dy) * COLS * CELL + col * CELL + dx) * 4
						);
			}
		}
		ctx.putImageData(image, 0, 0);
	});

	let tapped = $state<string | null>(null);
	let touch = false;

	function choose() {
		if (!hover) return;
		const again = tapped === hover.code;
		tapped = hover.code;
		if (!touch || again) pick(hover.code);
	}

	function locate(event: PointerEvent & { currentTarget: Element }) {
		touch = event.pointerType !== "mouse";
		const box = event.currentTarget.getBoundingClientRect();
		const col = Math.floor(((event.clientX - box.left) / box.width) * COLS);
		const row = Math.floor(((event.clientY - box.top) / box.height) * ROWS);
		const country =
			theme.style === "plain"
				? ((event.target as Element).getAttribute("data-code") ?? "")
				: code(col, row);
		hover = country
			? { code: country, x: event.clientX - box.left, y: event.clientY - box.top }
			: null;
	}
</script>

<div class="relative w-full">
	{#if theme.style === "plain"}
		{#if shapes}
			<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
			<svg
				viewBox="0 0 360 142"
				class="block w-full {hover ? 'cursor-pointer' : ''}"
				aria-label="visitors by country"
				onpointerdown={locate}
				onpointermove={locate}
				onpointerleave={(event) => event.pointerType === "mouse" && (hover = null)}
				onclick={choose}
			>
				{#each Object.entries(shapes) as [country, d] (country)}
					<path
						{d}
						data-code={country}
						stroke="var(--color-panel)"
						stroke-width="1"
						stroke-linejoin="round"
						vector-effect="non-scaling-stroke"
						style:fill={hover?.code === country
							? "var(--color-fg)"
							: `color-mix(in srgb, var(--color-cyan) ${share(country) * 100}%, var(--color-edge))`}
					/>
				{/each}
			</svg>
		{:else}
			<div class="aspect-[360/142]"></div>
		{/if}
	{:else}
		<canvas
			bind:this={canvas}
			width={COLS * CELL}
			height={ROWS * CELL}
			class="block w-full pixelated {hover ? 'cursor-pointer' : ''}"
			aria-label="visitors by country"
			onpointerdown={locate}
			onpointermove={locate}
			onpointerleave={(event) => event.pointerType === "mouse" && (hover = null)}
			onclick={choose}
		></canvas>
	{/if}
	{#if hover}
		<div
			class="pointer-events-none absolute z-10 flex -translate-x-1/2 -translate-y-full items-center gap-3 rounded-px border border-edge bg-raised px-3 py-2 whitespace-nowrap"
			style:left="{hover.x}px"
			style:top="{hover.y - 8}px"
		>
			<span class="text-small text-fg">{label(hover.code)}</span>
			<span class="label text-cyan">{format(values[hover.code] ?? 0)}</span>
		</div>
	{/if}
</div>
