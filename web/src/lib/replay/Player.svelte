<script lang="ts">
	import { Replayer } from "@rrweb/replay";
	import "@rrweb/replay/dist/style.css";
	import pause from "pixelarticons/svg/pause.svg?raw";
	import play from "pixelarticons/svg/play.svg?raw";
	import { css } from "#lib/dither/bayer.ts";
	import Button from "#lib/ui/Button.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { elapsed } from "./format.ts";
	import Timeline from "./Timeline.svelte";

	type Event = { type: number; timestamp: number; data?: { source?: number; type?: number } };

	let { events }: { events: Event[] } = $props();

	const SPEEDS = [1, 2, 4, 8];

	let root: HTMLDivElement;
	let replayer = $state.raw<Replayer | null>(null);
	let box = $state(0);
	let tall = $state(800);
	let size = $state({ width: 1280, height: 800 });
	let playing = $state(false);
	let time = $state(0);
	let speed = $state(1);
	let skip = $state<"skip" | "keep">("skip");
	let frame = 0;

	const start = $derived(events[0]?.timestamp ?? 0);
	const total = $derived((events.at(-1)?.timestamp ?? start) - start);
	// the whole player stays on screen with its controls
	const scale = $derived(Math.min(1, box / size.width, (tall * 0.65) / size.height));
	// rrweb: 3 is a change, its source 2 type 2 a click, and 4 a page load
	const marks = $derived({
		activity: events
			.filter((event) => event.type === 3)
			.map((event) => event.timestamp - start),
		pages: events.filter((event) => event.type === 4).map((event) => event.timestamp - start),
		clicks: events
			.filter(
				(event) => event.type === 3 && event.data?.source === 2 && event.data.type === 2
			)
			.map((event) => event.timestamp - start)
	});

	$effect(() => {
		const next = new Replayer(events as ConstructorParameters<typeof Replayer>[0], {
			root,
			skipInactive: true,
			showWarning: false,
			triggerFocus: false,
			mouseTail: { strokeStyle: css("--color-cyan"), lineWidth: 2, duration: 600 }
		});
		next.on("resize", (dimension) => (size = dimension as typeof size));
		next.on("finish", () => (playing = false));
		next.pause(0);
		replayer = next;
		return () => {
			cancelAnimationFrame(frame);
			next.destroy();
		};
	});

	$effect(() => {
		replayer?.setConfig({ speed, skipInactive: skip === "skip" });
	});

	function follow() {
		if (!replayer || !playing) return;
		time = replayer.getCurrentTime();
		frame = requestAnimationFrame(follow);
	}

	function toggle() {
		if (!replayer) return;
		if (playing) {
			replayer.pause();
			playing = false;
			return;
		}
		replayer.play(time >= total ? 0 : time);
		playing = true;
		follow();
	}

	function seek(ms: number) {
		time = ms;
		if (playing) replayer?.play(ms);
		else replayer?.pause(ms);
	}
</script>

<svelte:window
	bind:innerHeight={tall}
	onkeydown={(event) => {
		if (event.key !== " " || event.target !== document.body) return;
		event.preventDefault();
		toggle();
	}}
/>

<section class="flex flex-col rounded-px border border-line bg-panel">
	<div bind:clientWidth={box} class="overflow-hidden border-b border-line bg-ink">
		<div
			class="relative mx-auto"
			style:width="{size.width * scale}px"
			style:height="{size.height * scale}px"
		>
			<div
				bind:this={root}
				class="origin-top-left [&_.replayer-mouse::after]:bg-cyan! [&_iframe]:border-0"
				style:width="{size.width}px"
				style:height="{size.height}px"
				style:transform="scale({scale})"
			></div>
		</div>
	</div>
	<div class="flex flex-col gap-3 px-4 pt-5 pb-4">
		<Timeline {...marks} {total} {time} {seek} />
		<div class="flex flex-wrap items-center gap-3">
			<Button variant="icon" aria-label={playing ? "Pause" : "Play"} onclick={toggle}>
				<Icon svg={playing ? pause : play} size={16} />
			</Button>
			<span class="num text-small text-fg">{elapsed(time)}</span>
			<span class="num text-small text-faint">{elapsed(total)}</span>
			<div class="ml-auto flex items-center gap-3">
				<Toggle
					bind:value={skip}
					options={[
						{ key: "skip", label: "skip idle" },
						{ key: "keep", label: "real time" }
					]}
				/>
				<Select
					align="right"
					value={speed}
					options={SPEEDS.map((key) => ({ key, label: `${key}x` }))}
					data
					pick={(key) => (speed = key)}
				/>
			</div>
		</div>
	</div>
</section>
