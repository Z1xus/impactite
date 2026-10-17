import { api } from "./api.ts";

// a list the server sends back whole after every change, as `{ [key]: items }`
export function collection<T>(url: () => string, key: string) {
	let items = $state<T[] | null>(null);
	const keep = (reply: Record<string, T[]>) => void (items = reply[key]);

	return {
		get items() {
			return items;
		},
		load: () => api<Record<string, T[]>>(url()).then(keep),
		add: (body: unknown) => api<Record<string, T[]>>(url(), body).then(keep),
		change: (id: string | number, body: unknown) =>
			api<Record<string, T[]>>(`${url()}/${id}`, body, "PATCH").then(keep),
		remove: (id: string | number) =>
			api<Record<string, T[]>>(`${url()}/${id}`, undefined, "DELETE").then(keep)
	};
}
