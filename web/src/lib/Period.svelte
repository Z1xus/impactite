<script lang="ts">
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import calendar from "pixelarticons/svg/calendar.svg?raw";
	import left from "pixelarticons/svg/chevron-left.svg?raw";
	import right from "pixelarticons/svg/chevron-right.svg?raw";
	import { PRESETS, label, preset, step, type View } from "#lib/period.ts";
	import Calendar from "#lib/ui/Calendar.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import { fit } from "#lib/ui/fit.ts";
	import Icon from "#lib/ui/Icon.svelte";
	import Select from "#lib/ui/Select.svelte";

	let { view, go }: { view: View; go: (params: Record<string, string | null>) => void } =
		$props();

	const COMPARE = [
		{ key: "off", label: "No comparison" },
		{ key: "previous", label: "Previous period" },
		{ key: "year", label: "Year over year" },
		{ key: "custom", label: "Custom comparison" }
	];
	const CLEAR = { period: null, date: null, from: null, to: null, interval: null };

	let picking = $state<"menu" | "range" | "compare" | null>(null);
	let root: HTMLDivElement;

	const back = $derived(step(view, -1));
	const forward = $derived(step(view, 1));
	const rangeLabel = $derived(label(view));
	const selected = $derived(
		PRESETS.find((item) => {
			const params = preset(item.key, view);
			return params.period === view.period && (params.date ?? view.today) === view.date;
		})?.key ?? "custom"
	);

	function choose(key: string) {
		if (key === "custom") picking = "range";
		else {
			picking = null;
			go({ ...CLEAR, ...preset(key, view) });
		}
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
	<button
		type="button"
		aria-expanded={picking !== null}
		aria-label="Date range"
		class="flex h-9 cursor-pointer items-center gap-2 rounded-px px-2 font-sans text-[14px] text-fg hover:bg-raised"
		onclick={() => (picking = picking ? null : "menu")}
	>
		<Icon svg={calendar} size={14} class="text-dim" />
		{rangeLabel.charAt(0).toUpperCase() + rangeLabel.slice(1)}
		<Icon svg={chevron} size={12} class="text-dim" />
	</button>
	{#if view.compare !== "off" && view.period !== "all" && view.period !== "realtime"}
		<span class="hidden text-[13px] text-dim xl:inline"
			>vs {COMPARE.find((item) => item.key === view.compare)?.label}</span
		>
	{/if}
	{#if picking}
		<div
			{@attach fit}
			class="absolute top-full right-0 z-30 mt-1 rounded-px border border-edge bg-raised p-3"
		>
			{#if picking === "menu"}
				<div class="flex min-w-60 flex-col">
					{#each [...PRESETS, { key: "custom", label: "Custom range", hotkey: "c" }] as item (item.key)}
						{#if ["7d", "month", "all"].includes(item.key)}
							<span class="my-1 border-t border-line"></span>
						{/if}
						<button
							type="button"
							class="flex h-8 cursor-pointer items-center justify-between gap-6 rounded-px px-3 text-left text-[14px] hover:bg-line {selected ===
							item.key
								? 'text-cyan'
								: 'text-fg'}"
							onclick={() => choose(item.key)}
						>
							{item.label.charAt(0).toUpperCase() + item.label.slice(1)}
							{#if item.hotkey}
								<kbd class="label text-faint">{item.hotkey}</kbd>
							{/if}
						</button>
					{/each}
				</div>
				{#if view.period !== "all" && view.period !== "realtime"}
					<div class="mt-3 flex flex-col gap-3 border-t border-line pt-3">
						<Select
							label="compare with"
							value={view.compare}
							options={COMPARE}
							pick={compare}
							data
						/>
						{#if view.compare === "previous" || view.compare === "year"}
							<Checks
								bind:value={
									() => (view.weekday ? ["weekday"] : []),
									(value) => go({ match: value.length ? "weekday" : null })
								}
								options={[{ key: "weekday", label: "match weekday" }]}
							/>
						{/if}
					</div>
				{/if}
			{:else if picking === "range"}
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
