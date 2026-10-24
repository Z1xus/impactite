<script lang="ts">
	import { paint } from "#lib/dither/pixels.ts";
	import { PALETTE } from "#lib/art/meteor.ts";
	import { normalize, sprite, type Shape, type Vec } from "./sprite.ts";

	let {
		shape,
		level = 1,
		size = 256,
		pixel = 4,
		poke
	}: {
		shape: Shape;
		level?: number;
		size?: number;
		pixel?: number;
		poke?: () => void;
	} = $props();

	const LIGHT = normalize([-0.55, 0.6, 0.58]);
	const HOP = [2, 3, 3, 2, 1];
	const TWIST = 1.1;
	const n = $derived(Math.round(size / pixel));

	let canvas: HTMLCanvasElement;
	let lift = $state(0);
	let current = 1;
	let redraw = () => {};
	let turn = 0;
	let spin = 0;
	let held = false;
	let hopped = -1e4;

	$effect(() => {
		current = level;
		redraw();
	});

	$effect(() => {
		const render = sprite(shape, n);
		canvas.width = n;
		canvas.height = n;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;
		let light: Vec = LIGHT;
		let aim: Vec = LIGHT;
		let frame = 0;
		let last = 0;
		const draw = (now: number) => {
			const s = now / 1000;
			const hop = Math.floor((now - hopped) / 70);
			lift = HOP[hop] ?? (Math.sin(s * 1.9) > 0.35 ? 1 : 0);
			const angle = shape.pose + turn + Math.sin(s * 0.55) * shape.sway;
			paint(ctx, render(angle, light, now, hop < 8 ? 1 : current), n, PALETTE);
		};
		const tick = (now: number) => {
			const busy = held || Math.abs(turn) > 0.01 || now - hopped < 600;
			if (now - last >= (busy ? 33 : 83)) {
				last = now;
				// coast after a drag, then settle back on the pose
				if (!held) {
					turn = (turn + spin) * 0.88;
					spin *= 0.8;
				}
				light = normalize(light.map((v, k) => v + (aim[k] - v) * 0.25));
				draw(now);
			}
			frame = requestAnimationFrame(tick);
		};
		const point = (event: PointerEvent) => {
			if (event.pointerType !== "mouse") return;
			const box = canvas.getBoundingClientRect();
			const reach = Math.max(box.width, 240) * 1.5;
			const clamp = (v: number) => Math.max(-1, Math.min(1, v));
			aim = normalize([
				clamp((event.clientX - box.left - box.width / 2) / reach),
				-clamp((event.clientY - box.top - box.height / 2) / reach),
				0.55
			]);
		};
		redraw = () => draw(performance.now());
		draw(0);
		if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
		const observer = new IntersectionObserver(([entry]) => {
			cancelAnimationFrame(frame);
			if (entry.isIntersecting) frame = requestAnimationFrame(tick);
		});
		observer.observe(canvas);
		addEventListener("pointermove", point);
		return () => {
			observer.disconnect();
			cancelAnimationFrame(frame);
			removeEventListener("pointermove", point);
		};
	});

	function grab(event: PointerEvent) {
		canvas.setPointerCapture(event.pointerId);
		held = true;
		spin = 0;
		let x = event.clientX;
		let moved = 0;
		const listen = new AbortController();
		const options = { signal: listen.signal };
		const release = () => {
			held = false;
			listen.abort();
		};
		canvas.addEventListener(
			"pointermove",
			(move) => {
				const dx = move.clientX - x;
				x = move.clientX;
				moved += Math.abs(dx);
				spin = dx * 0.014;
				turn = Math.max(-TWIST, Math.min(TWIST, turn + spin));
				redraw();
			},
			options
		);
		canvas.addEventListener("pointercancel", release, options);
		canvas.addEventListener(
			"pointerup",
			() => {
				release();
				if (moved > 4) return;
				spin = 0;
				hopped = performance.now();
				poke?.();
				redraw();
			},
			options
		);
	}
</script>

<canvas
	bind:this={canvas}
	class="block max-w-full cursor-grab touch-pan-y pixelated active:cursor-grabbing"
	style:width="{size}px"
	style:translate="0 {-lift * pixel}px"
	aria-hidden="true"
	onpointerdown={grab}
></canvas>
