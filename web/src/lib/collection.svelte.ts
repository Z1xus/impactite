import { api, get } from "./api.ts";

// a list the server sends back whole after every change, as `{ [key]: items }`
export function collection<T>(url: () => string, key: string, query: () => string = () => "") {
	let items = $state.raw<T[] | null>(null);
	let extra = $state.raw<Record<string, unknown> | null>(null);
	let failed = $state(false);
	const keep = <R>(reply: Record<string, T[]> & R) => {
		items = reply[key];
		extra = reply;
		return reply;
	};

	return {
		get items() {
			return items;
		},
		get extra() {
			return extra;
		},
		get failed() {
			return failed;
		},
		keep,
		load: () => {
			failed = false;
			return get<Record<string, T[]>>(`${url()}${query()}`)
				.then(keep)
				.catch(() => {
					failed = true;
				});
		},
		add: <R = unknown>(body: unknown) => api<Record<string, T[]> & R>(url(), body).then(keep),
		change: (id: string | number, body: unknown) =>
			api<Record<string, T[]>>(
				`${url()}/${encodeURIComponent(id)}${query()}`,
				body,
				"PATCH"
			).then(keep),
		remove: (id: string | number) =>
			api<Record<string, T[]>>(
				`${url()}/${encodeURIComponent(id)}${query()}`,
				undefined,
				"DELETE"
			).then(keep)
	};
}
