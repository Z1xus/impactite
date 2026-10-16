// read once when the page is built, so visitors never contact github
export async function load({ fetch }) {
	const repo = await fetch("https://api.github.com/repos/Z1xus/impactite")
		.then((response) => (response.ok ? response.json() : null))
		.catch(() => null);
	return { stars: typeof repo?.stargazers_count === "number" ? repo.stargazers_count : null };
}
