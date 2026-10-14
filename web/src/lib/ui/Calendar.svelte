<script lang="ts">
	import { untrack } from "svelte";
	import left from "pixelarticons/svg/chevron-left.svg?raw";
	import right from "pixelarticons/svg/chevron-right.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		from,
		to,
		max,
		pick
	}: { from: string; to: string; max: string; pick: (from: string, to: string) => void } =
		$props();

	const DAYS = ["mo", "tu", "we", "th", "fr", "sa", "su"];

	let month = $state(untrack(() => to.slice(0, 7)));
	let start = $state<string | null>(null);
	let hover = $state<string | null>(null);

	const days = $derived.by(() => {
		const first = new Date(`${month}-01T00:00:00Z`);
		const blanks = (first.getUTCDay() + 6) % 7;
		const count = new Date(
			Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + 1, 0)
		).getUTCDate();
		return [
			...Array.from({ length: blanks }, () => null),
			...Array.from({ length: count }, (_, i) => `${month}-${String(i + 1).padStart(2, "0")}`)
		];
	});
	const title = $derived(
		new Date(`${month}-01T00:00:00Z`).toLocaleDateString("en-US", {
			timeZone: "UTC",
			month: "long",
			year: "numeric"
		})
	);
	const bounds = $derived.by(() => {
		if (!start) return [from, to];
		const end = hover ?? start;
		return end < start ? [end, start] : [start, end];
	});

	function move(months: number) {
		const [year, index] = month.split("-").map(Number);
		month = new Date(Date.UTC(year, index - 1 + months, 1)).toISOString().slice(0, 7);
	}

	function choose(day: string) {
		if (!start) {
			start = day;
			return;
		}
		const [a, b] = day < start ? [day, start] : [start, day];
		start = null;
		pick(a, b);
	}
</script>

<div class="flex w-64 flex-col gap-3 select-none [--dither-gap:var(--color-raised)]">
	<div class="flex items-center justify-between">
		<button
			type="button"
			class="grid h-7 w-7 cursor-pointer place-items-center text-dim hover:text-fg"
			aria-label="previous month"
			title="previous month"
			onclick={() => move(-1)}
		>
			<Icon svg={left} size={12} />
		</button>
		<span class="label text-fg">{title}</span>
		<button
			type="button"
			class="grid h-7 w-7 cursor-pointer place-items-center text-dim hover:text-fg disabled:cursor-default disabled:opacity-30"
			aria-label="next month"
			title="next month"
			disabled={month >= max.slice(0, 7)}
			onclick={() => move(1)}
		>
			<Icon svg={right} size={12} />
		</button>
	</div>
	<div class="grid grid-cols-7 gap-y-1">
		{#each DAYS as day (day)}
			<span class="grid h-6 place-items-center label">{day}</span>
		{/each}
		{#each days as day, i (day ?? `blank${i}`)}
			{#if day}
				{@const edge = day === bounds[0] || day === bounds[1]}
				{@const inside = day > bounds[0] && day < bounds[1]}
				<button
					type="button"
					class="grid h-8 cursor-pointer place-items-center font-mono text-[12px] transition-colors duration-75 disabled:cursor-default disabled:text-faint {edge
						? 'bg-cyan text-ink'
						: inside
							? 'dither text-fg'
							: 'text-dim hover:text-fg'}"
					disabled={day > max}
					onclick={() => choose(day)}
					onpointerenter={() => (hover = day)}
				>
					{Number(day.slice(8))}
				</button>
			{:else}
				<span></span>
			{/if}
		{/each}
	</div>
</div>
