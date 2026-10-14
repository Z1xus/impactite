export type Site = { id: number; domain: string; timezone: string };
export type Team = { id: number; name: string; personal: boolean; role: string; sites: Site[] };
export type Me = { id: number; name: string; email: string; teams: Team[] };

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
	}
}

export async function api<T>(path: string, body?: unknown, method?: string): Promise<T> {
	const res = await fetch(path, {
		method: method ?? (body === undefined ? "GET" : "POST"),
		headers: body === undefined ? undefined : { "content-type": "application/json" },
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	if (res.status === 204) return undefined as T;
	const data = await res.json().catch(() => ({}));
	if (!res.ok) throw new ApiError(res.status, data.error ?? "Something went wrong.");
	return data as T;
}
