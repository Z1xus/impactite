export type Site = {
	id: number;
	domain: string;
	timezone: string;
	retention: number;
	role: string;
};
export type Team = { id: number; name: string; personal: boolean; role: string; sites: Site[] };
export type Me = {
	id: number;
	name: string;
	email: string;
	password: boolean;
	admin: boolean;
	teams: Team[];
};

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
	}
}

export async function api<T>(path: string, body?: unknown, method?: string): Promise<T> {
	const json = body !== undefined && !(body instanceof Blob);
	const res = await fetch(path, {
		method: method ?? (body === undefined ? "GET" : "POST"),
		headers: json ? { "content-type": "application/json" } : undefined,
		body: json ? JSON.stringify(body) : (body as Blob | undefined)
	});
	if (res.status === 204) return undefined as T;
	const data = await res.json().catch(() => ({}));
	if (!res.ok) throw new ApiError(res.status, data.error ?? "Something went wrong.");
	return data as T;
}
