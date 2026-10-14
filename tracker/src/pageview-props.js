m.push((t) => {
	const props = {};
	for (const { name, value } of document.currentScript.attributes)
		if (name.startsWith("event-")) props[name.slice(6)] = value;
	const track = t.track;
	t.track = (name, o = {}) =>
		track(
			name,
			name == "pageview" ? { ...o, props: { ...props, ...o.props } } : o,
		);
});
