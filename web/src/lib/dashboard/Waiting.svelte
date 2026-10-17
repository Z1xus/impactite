<script lang="ts">
	import { api } from "#lib/api.ts";
	import { css, threshold } from "#lib/dither/bayer.ts";
	import { crater, random } from "#lib/dither/crater.ts";
	import Snippet from "#lib/settings/Snippet.svelte";
	import { display, query } from "#lib/stats.ts";
	import { theme } from "#lib/theme.svelte.ts";
	import Note from "#lib/ui/Note.svelte";

	let { domain, landed }: { domain: string; landed: () => void } = $props();

	const [WIDTH, HEIGHT] = [160, 48];
	let canvas = $state<HTMLCanvasElement>();
	let hit = $state<number | null>(null);
	let arrived = $state(false);
	let first = $state("");
	let strays = $state<string[]>([]);

	$effect(() => {
		const site = domain;
		let stopped = false;
		let pending = false;
		const timer = setInterval(async () => {
			if (pending) return;
			pending = true;
			const result = await query(
				site,
				{ metrics: ["events"], date_range: "all" },
				false
			).finally(() => (pending = false));
			if (stopped || !result.results[0]?.metrics[0]) return;
			clearInterval(timer);
			const [page, country] = await Promise.all(
				["event:page", "visit:country"].map((dim) =>
					query(
						site,
						{ metrics: ["visitors"], date_range: "all", dimensions: [dim] },
						false
					)
				)
			);
			if (stopped) return;
			const path = page.results[0]?.dimensions[0] ?? "";
			const code = country.results[0]?.dimensions[0] ?? "";
			const place = /^[A-Z]{2}$/.test(code) ? ` from ${display("visit:country", code)}` : "";
			first = path ? `Someone opened ${path}${place}.` : "";
			arrived = true;
		}, 2000);
		return () => {
			stopped = true;
			clearInterval(timer);
		};
	});

	$effect(() => {
		if (!arrived) return;
		let elapsed = 0;
		let started: number | null = null;
		let timer: ReturnType<typeof setTimeout>;
		const visibility = () => {
			clearTimeout(timer);
			if (started !== null) elapsed += performance.now() - started;
			started = null;
			if (document.hidden) return;
			started = performance.now();
			hit = started - elapsed;
			timer = setTimeout(landed, Math.max(0, 2600 - elapsed));
		};
		document.addEventListener("visibilitychange", visibility);
		visibility();
		return () => {
			clearTimeout(timer);
			document.removeEventListener("visibilitychange", visibility);
		};
	});

	$effect(() => {
		const site = domain;
		const look = () =>
			api<{ domains: string[] }>(`/api/app/sites/${site}/strays`)
				.then((r) => (strays = r.domains))
				.catch(() => {});
		const timer = setInterval(look, 5000);
		look();
		return () => clearInterval(timer);
	});

	$effect(() => {
		void theme.shown;
		const impact = hit;
		const g = canvas?.getContext("2d");
		if (!g) return;
		const [panel, night, deep, cyan, fg] = [
			"--color-panel",
			"--color-cyan-night",
			"--color-cyan-deep",
			"--color-cyan",
			"--color-fg"
		].map(css);
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		const rays = Array.from({ length: 7 }, random(domain.length + 7));
		const [cx, cy] = [WIDTH / 2, HEIGHT / 2 + 4];
		let frame = 0;
		const draw = (now: number) => {
			g.fillStyle = panel;
			g.fillRect(0, 0, WIDTH, HEIGHT);
			const since = impact === null ? -1 : still ? 9 : (now - impact) / 1000;
			if (since < 0) {
				// a slow scan line while nothing has arrived
				const at = (((now / 1000) * 24) % (WIDTH + 28)) - 14;
				for (let x = 0; x < WIDTH; x++) {
					const glow = Math.max(0, 1 - Math.abs(x - at) / 14) * 0.9;
					for (const y of [cy - 1, cy])
						if (glow > threshold(x, y)) {
							g.fillStyle = Math.abs(x - at) < 1 ? cyan : deep;
							g.fillRect(x, y, 1, 1);
						}
				}
			} else if (since < 0.55) {
				// the meteor falls in from the upper right
				const p = since / 0.55;
				const [x, y] = [cx + 40 * (1 - p), cy - 14 * (1 - p)];
				for (let i = 0; i < 16; i++) {
					const [tx, ty] = [Math.round(x + i * 2.6), Math.round(y - i * 0.9)];
					if (1 - i / 16 > threshold(tx, ty)) {
						g.fillStyle = i < 2 ? fg : cyan;
						g.fillRect(tx, ty, 1, 1);
					}
				}
			} else {
				const p = Math.min(1, (since - 0.55) / 0.5);
				crater(
					g,
					[WIDTH, HEIGHT],
					[cx, cy, 2 + 5 * Math.sqrt(p)],
					Math.min(1, p * 1.4),
					rays,
					{
						night,
						deep,
						cyan
					}
				);
			}
			if (since < 1.2) frame = requestAnimationFrame(draw);
		};
		frame = requestAnimationFrame(draw);
		return () => cancelAnimationFrame(frame);
	});
</script>

<section class="mx-auto flex w-full max-w-2xl flex-col gap-4 py-8 sm:py-12">
	<div
		class="flex flex-col items-center gap-4 rounded-px border border-line bg-panel p-6 text-center"
	>
		<canvas
			bind:this={canvas}
			width={WIDTH}
			height={HEIGHT}
			class="w-full pixelated"
			aria-hidden="true"
		></canvas>
		{#if hit === null}
			<span class="label">listening for the first visit on {domain}</span>
		{:else}
			<span class="flex flex-col gap-1">
				<span class="text-[14px] text-fg">First visit landed.</span>
				{#if first}<Note>{first}</Note>{/if}
			</span>
		{/if}
	</div>
	{#if hit === null}
		<div class="flex flex-col gap-3">
			<span class="text-[14px] text-dim"
				>Add this snippet to the head of every page on {domain}.</span
			>
			<Snippet {domain} />
		</div>
		<span class="text-[14px] text-dim"
			>Then open your site. Visits from localhost don't count.</span
		>
		{#each strays as stray (stray)}
			<span class="text-[14px] text-warn"
				>Visits are arriving for {stray}. Set data-domain to {domain} in the snippet, or add {stray}
				as its own site.</span
			>
		{/each}
	{/if}
</section>
