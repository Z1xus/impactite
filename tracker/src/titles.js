m.push((t) => {
	const send = t.send;
	t.send = (name, o) => {
		t.extra.t = name == "pageview" ? document.title : undefined;
		send(name, o);
	};
});
