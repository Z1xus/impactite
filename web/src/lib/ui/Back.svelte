<script lang="ts">
	import type { Snippet } from "svelte";
	import back from "pixelarticons/svg/arrow-left.svg?raw";
	import Icon from "./Icon.svelte";

	let { href, title, children }: { href: string; title: string; children: Snippet } = $props();
</script>

<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
<a {href} {title} class="group flex h-control min-w-0 items-center gap-3 hover:text-cyan">
	<Icon svg={back} size={16} class="text-dim group-hover:text-cyan" />
	{@render children()}
</a>
<!-- eslint-enable svelte/no-navigation-without-resolve -->
