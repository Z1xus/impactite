import { api, get } from "./api.ts";

// a list the server sends back whole after every change, as `{ [key]: items }`
export function collection<T>(url: () => string, key: string) {
	let items = $state.raw<T[] | null>(null);
	let failed = $state(false);
	const keep = <R>(reply: Record<string, T[]> & R) => {
		items = reply[key];
		return reply;
	};

	return {
		get items() {
			return items;
		},
		get failed() {
			return failed;
		},
		load: () => {
			failed = false;
			return get<Record<string, T[]>>(url())
				.then(keep)
				.catch(() => {
					failed = true;
				});
		},
		add: <R = unknown>(body: unknown) => api<Record<string, T[]> & R>(url(), body).then(keep),
		change: (id: string | number, body: unknown) =>
			api<Record<string, T[]>>(`${url()}/${encodeURIComponent(id)}`, body, "PATCH").then(
				keep
			),
		remove: (id: string | number) =>
			api<Record<string, T[]>>(
				`${url()}/${encodeURIComponent(id)}`,
				undefined,
				"DELETE"
			).then(keep)
	};
}
