m.push((t) => {
	const tagged = (event) => {
		const tag = event.target.closest?.("[class*='plausible-event-name=']");
		if (!tag) return;
		const props = {};
		let name;
		for (const token of tag.classList) {
			const [, key, value] =
				token.match(/^plausible-event-(.+?)=(.+)$/) || [];
			if (key == "name") name = value.replace(/\+/g, " ");
			else if (key) props[key] = value.replace(/\+/g, " ");
		}
		t.track(name, { props });
	};
	document.addEventListener("click", tagged);
	document.addEventListener("submit", tagged);
});
