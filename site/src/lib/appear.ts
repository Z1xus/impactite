// plays the entrance of each [data-appear] node once it scrolls into view
export function appear(root: HTMLElement) {
	const observer = new IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				if (!entry.isIntersecting) continue;
				entry.target.classList.add("shown");
				observer.unobserve(entry.target);
			}
		},
		{ threshold: 0.15 }
	);
	for (const node of root.querySelectorAll("[data-appear]:not(.shown)")) observer.observe(node);
	return { destroy: () => observer.disconnect() };
}
