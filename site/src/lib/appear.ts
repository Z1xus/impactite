import { BAYER } from "#lib/dither/bayer.ts";

const STEPS = 16;
const DURATION = 520;

// svg, so the cells stay sharp when scaled
const masks = Array.from({ length: STEPS + 1 }, (_, step) => {
	let cells = "";
	for (let i = 0; i < 64; i++)
		if (BAYER[i] < step / STEPS)
			cells += `<rect x='${i % 8}' y='${i >> 3}' width='1' height='1'/>`;
	const svg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 8 8'>${cells}</svg>`;
	return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
});

function dissolve(node: HTMLElement) {
	const delay = parseFloat(getComputedStyle(node).getPropertyValue("--delay")) || 0;
	node.style.maskSize = "32px 32px";
	node.style.maskImage = masks[0];
	node.classList.add("shown");
	const start = performance.now() + delay;
	const tick = (now: number) => {
		const step = Math.max(0, Math.min(STEPS, Math.floor(((now - start) / DURATION) * STEPS)));
		node.style.maskImage = masks[step];
		if (step < STEPS) requestAnimationFrame(tick);
		else node.style.maskImage = node.style.maskSize = "";
	};
	requestAnimationFrame(tick);
}

export function appear(root: HTMLElement) {
	if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
	const observer = new IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				if (!entry.isIntersecting) continue;
				dissolve(entry.target as HTMLElement);
				observer.unobserve(entry.target);
			}
		},
		{ threshold: 0.15 }
	);
	for (const node of root.querySelectorAll("[data-appear]:not(.shown)")) observer.observe(node);
	return () => observer.disconnect();
}
