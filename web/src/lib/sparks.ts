import type { Team } from "./api.ts";
import { shift, today } from "./period.ts";
import { teamTimezone } from "./session.svelte.ts";
import { cached } from "./stats.ts";

// the visitors of the last 28 days for each site of a team, and for the team as a whole
export async function sparks(team: Team) {
	const targets = [
		...(team.sites.length > 1
			? [{ key: "all", domain: `team:${team.id}`, timezone: teamTimezone(team) }]
			: []),
		...team.sites.map((site) => ({ key: site.domain, ...site }))
	];
	const series: Record<string, number[]> = {};
	let failed = false;
	await Promise.all(
		targets.map((target) =>
			cached(target.domain, {
				metrics: ["visitors"],
				date_range: [shift(today(target.timezone), -27), today(target.timezone)],
				dimensions: ["time:day"]
			})
				.then((result) => {
					series[target.key] = result.results.map((row) => row.metrics[0]);
				})
				.catch(() => {
					failed = true;
				})
		)
	);
	return { series, failed };
}
