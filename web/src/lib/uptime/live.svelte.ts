import { untrack } from "svelte";
import { api, get } from "#lib/api.ts";
import { swap } from "#lib/swap.ts";
import type { Check, Monitor } from "./types.ts";

export function stream(team: () => number | null, handle: (event: Check) => void) {
	$effect(() => {
		const id = team();
		if (!id) return;
		const source = new EventSource(`/api/app/uptime/live?team=${id}`);
		source.addEventListener("snapshot", (event) => {
			for (const item of JSON.parse(event.data) as Check[]) handle(item);
		});
		source.addEventListener("check", (event) => handle(JSON.parse(event.data) as Check));
		return () => source.close();
	});
}

export const order = (monitors: Monitor[]) =>
	[...monitors].sort(
		(a, b) =>
			Number(b.state === "down") - Number(a.state === "down") ||
			a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
	);

export function watching(team: () => number | null) {
	let monitors = $state.raw<Monitor[] | null>(null);
	let role = $state("");
	let today = $state(0);
	let failed = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let current = 0;

	const url = (id: number) => `/api/app/uptime?team=${id}`;

	function apply(next: Monitor[]) {
		const before =
			monitors &&
			order(monitors)
				.map((item) => item.id)
				.join();
		if (
			before !== null &&
			before !==
				order(next)
					.map((item) => item.id)
					.join()
		)
			swap(() => (monitors = next));
		else monitors = next;
	}

	async function load(fresh = false) {
		const id = team();
		if (!id) return;
		failed = false;
		try {
			const reply = await (fresh ? api : get)<{
				monitors: Monitor[];
				role: string;
				today: number;
			}>(url(id));
			if (id !== team()) return;
			role = reply.role;
			today = reply.today;
			apply(reply.monitors);
		} catch {
			if (!monitors) failed = true;
		}
	}

	function patch(event: Check) {
		const known = monitors?.find((item) => item.id === event.id);
		if (!monitors || !known) return;
		if (known.state !== event.state) {
			clearTimeout(timer);
			timer = setTimeout(() => load(true), 800);
		}
		if (
			known.state === event.state &&
			known.since === event.since &&
			known.latency === event.latency &&
			known.at === event.at &&
			known.cert_expires === event.cert_expires
		)
			return;
		monitors = monitors.map((item) => (item.id === event.id ? { ...item, ...event } : item));
	}

	$effect(() => {
		const id = team();
		if (id !== current) monitors = null;
		current = id ?? 0;
		untrack(() => load());
		const refresh = setInterval(() => load(true), 60_000);
		return () => {
			clearInterval(refresh);
			clearTimeout(timer);
		};
	});

	stream(team, patch);

	return {
		get monitors() {
			return monitors;
		},
		get role() {
			return role;
		},
		get today() {
			return today;
		},
		get failed() {
			return failed;
		},
		load: () => load(true)
	};
}
