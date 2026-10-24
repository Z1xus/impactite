<script lang="ts">
	import Split from "./Split.svelte";

	let {
		label,
		title,
		value,
		tone,
		shares
	}: {
		label: string;
		title: string;
		value: string;
		tone: "good" | "ni" | "poor";
		shares: number[];
	} = $props();

	const TONES = { good: "text-cyan", ni: "text-warn", poor: "text-bad" };
	const percent = (share: number) => `${Math.round(share * 100)}%`;
</script>

<div class="flex flex-col gap-3 px-4 py-4" {title}>
	<span class="label">{label}</span>
	<span class="num text-num whitespace-nowrap {TONES[tone]}">{value}</span>
	<div class="flex items-center gap-3">
		<Split
			{shares}
			label="{percent(shares[0])} good, {percent(shares[1])} needs work, {percent(
				shares[2]
			)} poor"
		/>
		<span class="num text-label text-dim">{percent(shares[0])}</span>
	</div>
</div>
