<script lang="ts">
	import Split from "./Split.svelte";

	let {
		label,
		title,
		value,
		tone,
		shares,
		active,
		pick
	}: {
		label: string;
		title: string;
		value: string;
		tone: "good" | "ni" | "poor";
		shares: number[];
		active: boolean;
		pick: () => void;
	} = $props();

	const TONES = { good: "text-cyan", ni: "text-warn", poor: "text-bad" };
	const percent = (share: number) => `${Math.round(share * 100)}%`;
</script>

<button
	type="button"
	class="flex flex-col gap-3 border-b px-4 py-4 text-left {active
		? 'border-cyan'
		: 'border-transparent hover:border-edge'}"
	{title}
	onclick={pick}
>
	<span class="label {active ? 'text-cyan' : ''}">{label}</span>
	<span class="num text-num whitespace-nowrap {TONES[tone]}">{value}</span>
	<span class="flex items-center gap-3">
		<Split
			{shares}
			labels={[
				`${percent(shares[0])} good`,
				`${percent(shares[1])} needs work`,
				`${percent(shares[2])} poor`
			]}
		/>
		<span class="num text-label text-dim">{percent(shares[0])} good</span>
	</span>
</button>
