m.push((t) =>
	document.addEventListener("submit", () =>
		t.track("Form: Submission", { props: { path: location.pathname } }),
	),
);
