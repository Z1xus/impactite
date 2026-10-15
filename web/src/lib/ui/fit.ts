// keeps a popup inside the viewport when its anchor sits near an edge
export function fit(node: HTMLElement) {
	const box = node.getBoundingClientRect();
	const shift = Math.max(8 - box.left, Math.min(0, innerWidth - 8 - box.right));
	if (shift) node.style.translate = `${shift}px`;
}
