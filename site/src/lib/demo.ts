import bell from "pixelarticons/svg/bell.svg?raw";
import coins from "pixelarticons/svg/coins.svg?raw";
import download from "pixelarticons/svg/download.svg?raw";
import filter from "pixelarticons/svg/filter.svg?raw";
import globe from "pixelarticons/svg/globe.svg?raw";
import key from "pixelarticons/svg/key.svg?raw";
import link from "pixelarticons/svg/link.svg?raw";
import mail from "pixelarticons/svg/mail.svg?raw";
import search from "pixelarticons/svg/search.svg?raw";
import shield from "pixelarticons/svg/shield.svg?raw";
import upload from "pixelarticons/svg/upload.svg?raw";
import users from "pixelarticons/svg/users.svg?raw";
import cloud from "pixelarticons/svg/cloud-server.svg?raw";
import grid from "pixelarticons/svg/grid-2x2-2.svg?raw";
import lock from "pixelarticons/svg/lock.svg?raw";
import target from "pixelarticons/svg/target.svg?raw";

export const REPO = "https://github.com/Z1xus/impactite";
export const BENCH = "https://github.com/Z1xus/impactite-benchmarks";
export const DEMO = import.meta.env.DEV
	? "http://localhost:5173/demo/impactite.dev"
	: "https://demo.impactite.dev/demo/impactite.dev";

// impactite-benchmarks, results/2026-10-15T130903Z.jsonl. impactite runs on its own api,
// which is plausible's, the others on theirs
export const STATS: {
	label: string;
	unit: string;
	better: "more" | "less";
	digits?: number;
	values: [number, number, number];
}[] = [
	{ label: "pageviews per second", unit: "/s", better: "more", values: [159799, 7753, 9738] },
	{ label: "top pages query", unit: "ms", better: "less", digits: 2, values: [0.34, 56.5, 1.34] },
	{
		label: "cpu for 1M pageviews",
		unit: "s",
		better: "less",
		digits: 1,
		values: [21.5, 1248.2, 137.9]
	},
	{ label: "idle memory", unit: "MiB", better: "less", values: [18, 942, 37] },
	{ label: "idle cpu", unit: "%", better: "less", digits: 2, values: [0, 2.69, 0.02] },
	{ label: "memory after 1M", unit: "MiB", better: "less", values: [124, 2217, 482] },
	{ label: "image size", unit: "MiB", better: "less", values: [19, 987, 36] },
	{ label: "tracker gzipped", unit: "B", better: "less", values: [930, 1324, 3259] }
];
export const TOOLS = ["impactite", "plausible", "goatcounter"];

export const FEATURES = [
	{ icon: target, label: "Goals and custom events" },
	{ icon: filter, label: "Funnels" },
	{ icon: coins, label: "Revenue tracking" },
	{ icon: search, label: "Google search keywords" },
	{ icon: link, label: "Shareable dashboards" },
	{ icon: mail, label: "Email and Slack reports" },
	{ icon: bell, label: "Traffic alerts" },
	{ icon: grid, label: "All sites in one view" },
	{ icon: upload, label: "Import from GA4 and Umami" },
	{ icon: download, label: "Full data export" },
	{ icon: cloud, label: "Daily exports to S3" },
	{ icon: shield, label: "Bot and IP filtering" },
	{ icon: globe, label: "No cookies" },
	{ icon: key, label: "Passkeys and two-factor" },
	{ icon: lock, label: "Single sign-on" },
	{ icon: users, label: "Teams and guest access" }
];

// 28 days of the demo site, with the day it hit the front page
const days = Array.from({ length: 28 }, (_, i) => {
	const weekly = [0.82, 1, 1.04, 1.02, 0.97, 0.7, 0.62][i % 7];
	const spike = i === 18 ? 4.3 : i === 19 ? 1.9 : 1;
	return Math.round((210 + i * 6) * weekly * spike);
});
export const LABELS = days.map((_, i) =>
	new Date(Date.UTC(2026, 8, 18 + i)).toLocaleDateString("en-US", {
		month: "short",
		day: "numeric",
		timeZone: "UTC"
	})
);

// the same six numbers, panels and tabs as the dashboard, filled with the demo site's month
export const TOTALS = [9148, 10512, 21400, 54, 127, 2.04];
export const PREVIOUS = [6629, 7729, 15620, 54, 130, 2.02];

// visits and pageviews follow the visitors line, the rates wander around their average
const FACTORS = [1, 1.15, 2.34];
export const shape = (i: number) =>
	i < FACTORS.length
		? days.map((day) => Math.round(day * FACTORS[i]))
		: days.map((_, k) => TOTALS[i] * (0.9 + 0.2 * Math.sin(k * 0.7 + i)));

export const PANELS = {
	left: {
		tabs: [
			{ key: "visit:source", label: "sources" },
			{ key: "visit:channel", label: "channels" },
			{ key: "visit:utm_source", label: "utm" }
		],
		rows: {
			"visit:source": [
				["Google", 3811],
				["Direct / None", 2506],
				["Hacker News", 586],
				["Twitter", 521],
				["Reddit", 517],
				["GitHub", 402],
				["Bing", 236],
				["DuckDuckGo", 198]
			],
			"visit:channel": [
				["Organic Search", 4180],
				["Direct", 2506],
				["Organic Social", 1038],
				["Referral", 1004],
				["Email", 420],
				["Paid Search", 212],
				["Organic Video", 96],
				["Display", 41]
			],
			"visit:utm_source": [
				["newsletter", 420],
				["github", 312],
				["mastodon", 118],
				["producthunt", 96],
				["lobsters", 74],
				["devto", 61],
				["youtube", 38],
				["bluesky", 27]
			]
		} as Record<string, [string, number][]>
	},
	right: {
		tabs: [
			{ key: "event:page", label: "pages" },
			{ key: "visit:entry_page", label: "entry" },
			{ key: "visit:exit_page", label: "exit" },
			{ key: "event:hostname", label: "hostnames" }
		],
		rows: {
			"event:page": [
				["/", 4377],
				["/pricing", 2185],
				["/blog/crater", 2165],
				["/docs", 1524],
				["/docs/install", 1360],
				["/blog", 902],
				["/docs/api", 611],
				["/changelog", 388]
			],
			"visit:entry_page": [
				["/", 5021],
				["/blog/crater", 1840],
				["/docs/install", 902],
				["/pricing", 611],
				["/docs", 400],
				["/blog", 312],
				["/changelog", 190],
				["/docs/api", 142]
			],
			"visit:exit_page": [
				["/docs/install", 2210],
				["/", 2012],
				["/pricing", 1630],
				["/blog/crater", 1402],
				["/docs", 980],
				["/blog", 640],
				["/docs/api", 402],
				["/changelog", 236]
			],
			"event:hostname": [
				["impactite.dev", 5810],
				["docs.impactite.dev", 1622],
				["blog.impactite.dev", 1104],
				["demo.impactite.dev", 842],
				["status.impactite.dev", 410],
				["app.impactite.dev", 318],
				["cdn.impactite.dev", 264],
				["play.impactite.dev", 142]
			]
		} as Record<string, [string, number][]>
	}
};

const HOST = "stats.example.com";
export const PLAUSIBLE = [
	`<script defer data-domain="example.com" src="https://`,
	{ from: "plausible.io", to: HOST },
	`/js/script.js"></script>`
];
export const GOATCOUNTER = [
	`<script data-goatcounter="https://`,
	{ from: "example.goatcounter.com", to: HOST },
	`/count" async src="`,
	{ from: "//gc.zgo.at", to: `https://${HOST}` },
	`/count.js"></script>`
];
