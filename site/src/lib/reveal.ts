// flips once when the element first scrolls into view
export function reveal(node: HTMLElement, shown: (value: boolean) => void) {
	const observer = new IntersectionObserver(
		([entry]) => {
			if (!entry.isIntersecting) return;
			shown(true);
			observer.disconnect();
		},
		{ threshold: 0.35 }
	);
	observer.observe(node);
	return { destroy: () => observer.disconnect() };
}
