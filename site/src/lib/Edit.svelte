<script lang="ts">
	import check from "pixelarticons/svg/check.svg?raw";
	import copy from "pixelarticons/svg/copy.svg?raw";
	import Icon from "#lib/ui/Icon.svelte";

	type Part = string | { from: string; to: string };

	let { parts, on, delay = 0 }: { parts: Part[]; on: boolean; delay?: number } = $props();

	const final = $derived(
		parts.map((part) => (typeof part === "string" ? part : part.to)).join("")
	);

	// per edit: the old text is selected, then replaced, then the new text is typed in
	let edits: Record<number, { selected: boolean; typed: number }> = $state({});
	let active = $state(-1);
	let copied = $state(false);
	let started = false;

	const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

	async function play() {
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		await wait(still ? 0 : delay);
		for (const [i, part] of parts.entries()) {
			if (typeof part === "string") continue;
			edits[i] = { selected: false, typed: -1 };
			if (still) {
				edits[i].typed = part.to.length;
				continue;
			}
			active = i;
			edits[i].selected = true;
			await wait(550);
			edits[i].selected = false;
			for (let typed = 0; typed <= part.to.length; typed++) {
				edits[i].typed = typed;
				await wait(38);
			}
			await wait(250);
		}
		active = -1;
	}

	$effect(() => {
		if (on && !started) {
			started = true;
			play();
		}
	});

	async function write() {
		await navigator.clipboard.writeText(final);
		copied = true;
		setTimeout(() => (copied = false), 1200);
	}
</script>

<div class="flex items-start gap-2 rounded-px border border-line bg-panel py-2.5 pr-2 pl-3">
	<code class="min-w-0 flex-1 py-0.5 font-mono text-[12px] leading-5 break-all text-cyan"
		>{#each parts as part, i (i)}{#if typeof part === "string"}{part}{:else if (edits[i]?.typed ?? -1) < 0}<span
					class={edits[i]?.selected ? "bg-edge text-fg" : ""}>{part.from}</span
				>{:else}<span class="bg-cyan-night text-fg">{part.to.slice(0, edits[i].typed)}</span
				>{/if}{#if active === i}<span
					class="inline-block h-[1.1em] w-[0.55em] translate-y-[0.2em] bg-cyan"
				></span>{/if}{/each}</code
	>
	<button
		type="button"
		class="grid h-6 w-6 shrink-0 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-fg"
		aria-label="copy"
		title="copy"
		onclick={write}
	>
		<Icon svg={copied ? check : copy} size={12} />
	</button>
</div>
