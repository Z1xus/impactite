// hides the node until it scrolls into view, then plays one stepped entrance
export function appear(
	node: HTMLElement,
	{ kind = "rise", delay = 0 }: { kind?: "rise" | "wipe"; delay?: number } = {}
) {
	if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
	node.style.opacity = "0";
	const observer = new IntersectionObserver(
		([entry]) => {
			if (!entry.isIntersecting) return;
			observer.disconnect();
			node.style.opacity = "";
			node.style.animation =
				kind === "wipe"
					? `wipe 900ms steps(18) ${delay}ms both`
					: `rise 420ms steps(6) ${delay}ms both`;
		},
		{ threshold: 0.15 }
	);
	observer.observe(node);
	return { destroy: () => observer.disconnect() };
}
