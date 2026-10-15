<script lang="ts">
	import calendar from "pixelarticons/svg/calendar.svg?raw";
	import left from "pixelarticons/svg/chevron-left.svg?raw";
	import right from "pixelarticons/svg/chevron-right.svg?raw";
	import { PRESETS, label, preset, step, type View } from "#lib/period.ts";
	import Calendar from "#lib/ui/Calendar.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Select from "#lib/ui/Select.svelte";

	let { view, go }: { view: View; go: (params: Record<string, string | null>) => void } =
		$props();

	const COMPARE = [
		{ key: "off", label: "no comparison" },
		{ key: "previous", label: "previous period" },
		{ key: "year", label: "year over year" },
		{ key: "custom", label: "custom comparison" }
	];
	const CLEAR = { period: null, date: null, from: null, to: null, interval: null };

	let picking = $state<"range" | "compare" | null>(null);
	let root: HTMLDivElement;

	const back = $derived(step(view, -1));
	const forward = $derived(step(view, 1));
	const selected = $derived(
		PRESETS.find((item) => {
			const params = preset(item.key, view);
			return params.period === view.period && (params.date ?? view.today) === view.date;
		})?.key ?? "custom"
	);

	function choose(key: string) {
		if (key === "custom") picking = "range";
		else go({ ...CLEAR, ...preset(key, view) });
	}

	function compare(key: string) {
		if (key === "custom") picking = "compare";
		else go({ compare: key === "off" ? null : key, compare_from: null, compare_to: null });
	}

	function keydown(event: KeyboardEvent) {
		const target = event.target as HTMLElement;
		if (event.metaKey || event.ctrlKey || event.altKey || target.closest("input, textarea"))
			return;
		const hotkey = PRESETS.find((item) => item.hotkey === event.key);
		if (hotkey) choose(hotkey.key);
		else if (event.key === "c") picking = "range";
		else if (event.key === "x") compare(view.compare === "off" ? "previous" : "off");
		else if (event.key === "ArrowLeft" && back) go({ date: back });
		else if (event.key === "ArrowRight" && forward) go({ date: forward });
		else if (event.key === "Escape") picking = null;
	}

	function outside(event: PointerEvent) {
		if (picking && !root.contains(event.target as Node)) picking = null;
	}
</script>

<svelte:window onkeydown={keydown} onpointerdown={outside} />

<div class="relative flex flex-wrap items-center gap-2" bind:this={root}>
	{#if back || forward}
		<div class="flex">
			<button
				type="button"
				class="grid h-9 w-7 cursor-pointer place-items-center text-dim hover:text-fg disabled:cursor-default disabled:opacity-30"
				aria-label="earlier"
				title="earlier"
				disabled={!back}
				onclick={() => back && go({ date: back })}
			>
				<Icon svg={left} size={12} />
			</button>
			<button
				type="button"
				class="grid h-9 w-7 cursor-pointer place-items-center text-dim hover:text-fg disabled:cursor-default disabled:opacity-30"
				aria-label="later"
				title="later"
				disabled={!forward}
				onclick={() => forward && go({ date: forward })}
			>
				<Icon svg={right} size={12} />
			</button>
		</div>
	{/if}
	<Select
		align="right"
		value={selected}
		text={label(view)}
		icon={calendar}
		options={[
			...PRESETS.map(({ key, label }) => ({ key, label })),
			{ key: "custom", label: "custom range" }
		]}
		pick={choose}
	/>
	{#if view.period !== "all" && view.period !== "realtime"}
		<Select align="right" value={view.compare} options={COMPARE} pick={compare} />
		{#if view.compare === "previous" || view.compare === "year"}
			<Checks
				bind:value={
					() => (view.weekday ? ["weekday"] : []),
					(value) => go({ match: value.length ? "weekday" : null })
				}
				options={[{ key: "weekday", label: "match weekday" }]}
			/>
		{/if}
	{/if}
	{#if picking}
		<div
			class="absolute top-full right-0 z-30 mt-1 rounded-px border border-edge bg-raised p-3"
		>
			{#if picking === "range"}
				<Calendar
					from={view.period === "custom" ? view.from : view.today}
					to={view.period === "custom" ? view.to : view.today}
					max={view.today}
					pick={(from, to) => {
						picking = null;
						go({ ...CLEAR, period: "custom", from, to });
					}}
				/>
			{:else}
				<Calendar
					from={view.compareFrom}
					to={view.compareTo}
					max={view.today}
					pick={(from, to) => {
						picking = null;
						go({ compare: "custom", compare_from: from, compare_to: to });
					}}
				/>
			{/if}
		</div>
	{/if}
</div>
