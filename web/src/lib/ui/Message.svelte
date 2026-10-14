<script lang="ts">
	import type { Snippet } from "svelte";

	let { tone = "dim", children }: { tone?: "dim" | "bad" | "cyan"; children?: Snippet } =
		$props();

	const tones = { dim: "text-dim", bad: "text-bad", cyan: "text-cyan" };
</script>

<p class="min-h-[11px] label normal-case {tones[tone]}" aria-live="polite">
	{@render children?.()}
</p>
