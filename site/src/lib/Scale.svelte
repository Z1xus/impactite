<script lang="ts">
	// from the benchmark: cpu seconds for a million pageviews, and pageviews a second on one machine
	let { cpu, rate }: { cpu: number; rate: number } = $props();

	let position = $state(50);

	const monthly = $derived(Math.round(10 ** (4 + (position / 100) * 6)));
	const daily = $derived((monthly / 30) * (cpu / 1e6));
	const core = $derived((daily / 86_400) * 100);
	// busy hours run at about ten times the average rate
	const room = $derived(Math.max(1, Math.round(rate / ((monthly / 30 / 86_400) * 10))));

	const count = (value: number) => Math.round(value).toLocaleString("en-US");
</script>

<div class="flex flex-col gap-8 rounded-px border border-line bg-panel p-5">
	<label class="flex flex-col gap-3">
		<span class="label">your pageviews a month</span>
		<span class="font-mono text-heading tracking-tight">{count(monthly)}</span>
		<input
			type="range"
			min="0"
			max="100"
			step="1"
			bind:value={position}
			aria-valuetext={count(monthly)}
			class="range"
		/>
	</label>
	<dl class="grid grid-cols-2 gap-6 sm:grid-cols-3">
		<div class="flex flex-col gap-2">
			<dt class="label">cpu a day</dt>
			<dd class="font-mono text-num">
				{daily < 60 ? `${daily.toFixed(1)} s` : `${(daily / 60).toFixed(1)} min`}
			</dd>
		</div>
		<div class="flex flex-col gap-2">
			<dt class="label">of one core</dt>
			<dd class="font-mono text-num">{core < 0.01 ? "<0.01" : core.toFixed(2)}%</dd>
		</div>
		<div class="flex flex-col gap-2">
			<dt class="label">headroom at peak</dt>
			<dd class="font-mono text-num">{count(room)}x</dd>
		</div>
	</dl>
	<p class="border-t border-line pt-5 text-body">
		{core < 5 ? "No. One small server is enough." : "Still one server. Give it more cores."}
	</p>
</div>
