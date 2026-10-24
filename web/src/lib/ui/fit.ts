export function fit(node: HTMLElement) {
	const box = node.getBoundingClientRect();
	const shift = Math.max(8 - box.left, Math.min(0, innerWidth - 8 - box.right));
	if (shift) node.style.translate = `${shift}px`;
	const below = innerHeight - 8 - box.top;
	if (box.height <= below) return;
	const above = (node.offsetParent?.getBoundingClientRect().top ?? 0) - 12;
	const flip = above > below;
	if (flip)
		Object.assign(node.style, {
			top: "auto",
			bottom: "100%",
			marginTop: "0",
			marginBottom: "4px"
		});
	node.style.maxHeight = `min(24rem, ${flip ? above : below}px)`;
}
