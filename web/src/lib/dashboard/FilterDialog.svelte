<script lang="ts">
	import { untrack } from "svelte";
	import { DIMENSIONS, OPS, display, query, type Filter, type Op } from "#lib/stats.ts";
	import type { Range } from "#lib/period.ts";
	import Button from "#lib/ui/Button.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";

	let {
		domain,
		range,
		goals,
		propKeys,
		initial,
		apply,
		remove
	}: {
		domain: string;
		range: Range;
		goals: string[];
		propKeys: string[];
		initial: Filter | null;
		apply: (filter: Filter) => void;
		remove: (dim: string) => void;
	} = $props();

	const options = $derived([
		...DIMENSIONS.filter((item) => item.key !== "event:goal" || goals.length),
		...propKeys.map((key) => ({ key: `event:props:${key}`, label: `prop ${key}` }))
	]);

	let dim = $state(untrack(() => initial?.dim ?? "event:page"));
	let op = $state<Op>(untrack(() => initial?.op ?? "is"));
	let values = $state<string[]>(untrack(() => initial?.values ?? []));
	let text = $state("");
	let suggestions = $state<string[]>([]);
	const goal = $derived(dim === "event:goal");
	const country = $derived(dim === "visit:country");

	$effect(() => {
		const typed = text.trim().toLowerCase();
		if (goal) {
			suggestions = goals.filter((name) => name.toLowerCase().includes(typed));
			return;
		}
		let stale = false;
		const timer = setTimeout(() => {
			query(domain, {
				metrics: ["visitors"],
				date_range: range === "30m" ? "day" : range,
				dimensions: [dim],
				filters: typed && !country ? [["contains", dim, [text.trim()]]] : [],
				pagination: { limit: country ? 300 : 12 }
			}).then((result) => {
				if (stale) return;
				suggestions = result.results
					.map((row) => row.dimensions[0])
					.filter(
						(value) => !country || display(dim, value).toLowerCase().includes(typed)
					)
					.slice(0, 12);
			});
		}, 120);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});

	function pickDim(next: string) {
		dim = next;
		values = [];
		text = "";
	}

	const chosen = $derived.by(() => {
		const typed = text.trim();
		if (goal) {
			const match = [...values, typed].filter((value) => goals.includes(value)).slice(-1);
			return match.length || !typed || !suggestions[0] ? match : [suggestions[0]];
		}
		return values.length ? values : typed ? [typed] : [];
	});

	function submit(event: SubmitEvent) {
		event.preventDefault();
		if (chosen.length) apply({ op: goal ? "is" : op, dim, values: chosen });
	}
</script>

<form class="flex flex-col gap-5" onsubmit={submit}>
	<div class="flex flex-wrap items-center gap-2">
		<Select value={dim} {options} pick={pickDim} />
		{#if !goal}
			<Toggle bind:value={op} options={OPS} />
		{/if}
	</div>
	<Field label={goal ? "goal" : "value"} placeholder="search" bind:value={text} autofocus />
	<Checks
		data
		bind:value={() => values, (next) => (values = goal ? next.slice(-1) : next)}
		options={[...new Set([...values, ...suggestions])].map((value) => ({
			key: value,
			label: display(dim, value)
		}))}
	/>
	<div class="flex items-center justify-between">
		{#if initial}
			<Button variant="plain" type="button" onclick={() => remove(initial.dim)}
				>Remove filter</Button
			>
		{:else}
			<span></span>
		{/if}
		<Button type="submit" disabled={!chosen.length}>Apply</Button>
	</div>
</form>
