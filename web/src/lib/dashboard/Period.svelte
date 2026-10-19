<script lang="ts">
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import { tick } from "svelte";
	import calendar from "pixelarticons/svg/calendar.svg?raw";
	import left from "pixelarticons/svg/chevron-left.svg?raw";
	import right from "pixelarticons/svg/chevron-right.svg?raw";
	import { PRESETS, label, preset, step, type View } from "#lib/period.ts";
	import Calendar from "#lib/ui/Calendar.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import { fit } from "#lib/ui/fit.ts";
	import { dismiss } from "#lib/ui/popover.ts";
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

	let root: HTMLDivElement;
	let toggle: HTMLButtonElement;
	let picking = $state<"menu" | "range" | "compare" | null>(null);

	const back = $derived(step(view, -1));
	const forward = $derived(step(view, 1));
	const rangeLabel = $derived(label(view));
	const selected = $derived(
		PRESETS.find((item) => {
			const params = preset(item.key, view);
			return params.period === view.period && (params.date ?? view.today) === view.date;
		})?.key ?? "custom"
	);

	async function arrows(event: KeyboardEvent) {
		const target = event.target as HTMLElement;
		const down = event.key === "ArrowDown";
		const edge = event.key === "Home" || event.key === "End";
		if (!down && !edge && event.key !== "ArrowUp") return;
		const trigger = target.hasAttribute("aria-expanded");
		if (!trigger && !target.closest("[data-presets]")) return;
		event.preventDefault();
		if (trigger && !picking) {
			picking = "menu";
			await tick();
		}
		const items = [...root.querySelectorAll<HTMLElement>("[data-presets] button")];
		const at = items.indexOf(target);
		const next = edge
			? event.key === "Home"
				? 0
				: -1
			: trigger && !down
				? -1
				: (at + (down ? 1 : -1)) % items.length;
		items.at(next)?.focus();
	}

	function close() {
		picking = null;
		toggle.focus();
	}

	function choose(key: string) {
		if (key === "custom") picking = "range";
		else {
			close();
			go({ ...CLEAR, ...preset(key, view) });
		}
	}

	function compare(key: string) {
		if (key === "custom") picking = "compare";
		else go({ compare: key === "off" ? null : key, compare_from: null, compare_to: null });
	}

	function keydown(event: KeyboardEvent) {
		const target = event.target as HTMLElement;
		if (
			event.metaKey ||
			event.ctrlKey ||
			event.altKey ||
			target.closest("input, textarea") ||
			document.querySelector("dialog[open]")
		)
			return;
		const hotkey = PRESETS.find((item) => item.hotkey === event.key);
		if (hotkey) choose(hotkey.key);
		else if (event.key === "c") picking = "range";
		else if (event.key === "x") compare(view.compare === "off" ? "previous" : "off");
		else if (event.key === "ArrowLeft" && back) go({ date: back });
		else if (event.key === "ArrowRight" && forward) go({ date: forward });
	}
</script>

<svelte:window onkeydown={keydown} />

<div
	class="relative flex flex-wrap items-center gap-2"
	bind:this={root}
	onkeydown={arrows}
	role="presentation"
	{@attach dismiss(
		() => picking !== null,
		() => (picking = null)
	)}
>
	{#if back || forward}
		<div class="flex">
			<button
				type="button"
				class="grid h-control w-8 place-items-center text-dim enabled:hover:text-fg"
				aria-label="earlier"
				title="earlier"
				disabled={!back}
				onclick={() => back && go({ date: back })}
			>
				<Icon svg={left} size={12} />
			</button>
			<button
				type="button"
				class="grid h-control w-8 place-items-center text-dim enabled:hover:text-fg"
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
		bind:this={toggle}
		type="button"
		aria-expanded={picking !== null}
		aria-label="Date range"
		class="flex h-control items-center gap-2 rounded-px px-2 font-sans text-body text-fg hover:bg-raised"
		onclick={() => (picking = picking ? null : "menu")}
	>
		<Icon svg={calendar} size={12} class="text-dim" />
		{rangeLabel.charAt(0).toUpperCase() + rangeLabel.slice(1)}
		{#if view.compare !== "off" && view.period !== "all" && view.period !== "realtime"}
			<span class="hidden text-small text-dim xl:inline"
				>vs {COMPARE.find((item) => item.key === view.compare)?.label}</span
			>
		{/if}
		<Icon svg={chevron} size={12} class="text-dim" />
	</button>
	{#key picking}
		{#if picking}
			<div {@attach fit} class="popover right-0 p-3">
				{#if picking === "menu"}
					<div class="flex min-w-60 flex-col" data-presets>
						{#each [...PRESETS, { key: "custom", label: "Custom range", hotkey: "c" }] as item (item.key)}
							{#if ["7d", "month", "all"].includes(item.key)}
								<span class="my-1 border-t border-line"></span>
							{/if}
							<button
								type="button"
								class="flex h-control items-center justify-between gap-6 rounded-px px-3 text-left text-body hover:bg-line {selected ===
								item.key
									? 'text-cyan'
									: 'text-fg'}"
								onclick={() => choose(item.key)}
							>
								{item.label.charAt(0).toUpperCase() + item.label.slice(1)}
								{#if item.hotkey}
									<kbd class="label text-faint pointer-coarse:hidden"
										>{item.hotkey}</kbd
									>
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
							close();
							go({ ...CLEAR, period: "custom", from, to });
						}}
					/>
				{:else}
					<Calendar
						from={view.compareFrom}
						to={view.compareTo}
						max={view.today}
						pick={(from, to) => {
							close();
							go({ compare: "custom", compare_from: from, compare_to: to });
						}}
					/>
				{/if}
			</div>
		{/if}
	{/key}
</div>
