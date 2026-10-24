<script lang="ts">
	import { Replayer } from "@rrweb/replay";
	import "@rrweb/replay/dist/style.css";
	import { localize } from "#lib/replay/assets.ts";

	let {
		domain,
		node,
		href,
		width,
		height,
		scale,
		ready
	}: {
		domain: string;
		node: unknown;
		href: string;
		width: number;
		height: number;
		scale: number;
		ready: (page: Document) => void;
	} = $props();

	let root: HTMLDivElement;

	// a stored page rebuilt in a sandboxed frame, never running its scripts
	$effect(() => {
		localize(domain, node);
		const replayer = new Replayer(
			[
				{ type: 4, data: { href, width, height }, timestamp: 0 },
				{ type: 2, data: { node, initialOffset: { top: 0, left: 0 } }, timestamp: 1 }
			] as ConstructorParameters<typeof Replayer>[0],
			{ root, mouseTail: false, showWarning: false, triggerFocus: false }
		);
		replayer.on("fullsnapshot-rebuilded", () => {
			if (replayer.iframe.contentDocument) ready(replayer.iframe.contentDocument);
		});
		replayer.pause(1);
		return () => replayer.destroy();
	});
</script>

<div
	bind:this={root}
	class="pointer-events-none origin-top-left [&_.replayer-mouse]:hidden [&_iframe]:border-0"
	style:width="{width}px"
	style:height="{height}px"
	style:transform="scale({scale})"
></div>
