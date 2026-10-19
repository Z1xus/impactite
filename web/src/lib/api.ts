export type Site = {
	id: number;
	domain: string;
	timezone: string;
	retention: number;
	counter: boolean;
	skip: string[];
	role: string;
};
export type Team = { id: number; name: string; personal: boolean; role: string; sites: Site[] };
export type Me = {
	id: number;
	name: string;
	email: string;
	password: boolean;
	admin: boolean;
	totp: boolean;
	teams: Team[];
};

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string,
		public data: Record<string, unknown> = {}
	) {
		super(message);
	}
}

// a share link token, sent so anonymous viewers can read one site
export const access = { share: "" };

type Reply = { status: number; body: unknown };
type Waiting = {
	path: string;
	method: string;
	body: unknown;
	done: (reply: Reply) => void;
	fail: (err: unknown) => void;
};

const READS = new Set(["/api/app/query", "/api/app/props"]);
let waiting: Waiting[] = [];

async function send(
	path: string,
	method: string,
	body?: BodyInit,
	json = false,
	retries = 0
): Promise<Reply> {
	try {
		const res = await fetch(path, {
			method,
			headers: {
				...(json && { "content-type": "application/json" }),
				...(access.share && { "x-share": access.share })
			},
			body,
			signal: retries ? AbortSignal.timeout(20_000) : undefined
		});
		return {
			status: res.status,
			body: res.status === 204 ? undefined : await res.json().catch(() => ({}))
		};
	} catch (err) {
		if (!retries) throw err;
		await new Promise((resolve) => setTimeout(resolve, 600));
		return send(path, method, body, json, retries - 1);
	}
}

// calls made in the same moment travel in one request
async function flush() {
	const calls = waiting;
	waiting = [];
	try {
		if (calls.length === 1) {
			const [{ path, method, body, done }] = calls;
			done(
				await send(
					path,
					method,
					body === undefined ? undefined : JSON.stringify(body),
					body !== undefined,
					2
				)
			);
			return;
		}
		const { body } = await send(
			"/api/app/batch",
			"POST",
			JSON.stringify(calls.map(({ path, method, body }) => ({ path, method, body }))),
			true,
			2
		);
		calls.forEach((call, i) => call.done((body as Reply[])[i]));
	} catch (err) {
		calls.forEach((call) => call.fail(err));
	}
}

function read(path: string, method: string, body: unknown) {
	return new Promise<Reply>((done, fail) => {
		if (!waiting.length) setTimeout(flush, 4);
		waiting.push({ path, method, body, done, fail });
	});
}

const gets = new Map<string, { at: number; reply: Promise<unknown> }>();
const KEEP = 10_000;

// a read shared for a few seconds, so a hover or a route load feeds the page it opens
export function get<T>(path: string, body?: unknown) {
	const key = JSON.stringify([path, body, access.share]);
	const hit = gets.get(key);
	if (hit && Date.now() - hit.at < KEEP) return hit.reply as Promise<T>;
	const reply = api<T>(path, body);
	gets.set(key, { at: Date.now(), reply });
	reply.catch(() => gets.delete(key));
	return reply;
}

export async function api<T>(path: string, body?: unknown, method?: string): Promise<T> {
	const json = body !== undefined && !(body instanceof Blob);
	const verb = method ?? (body === undefined ? "GET" : "POST");
	const reading = (verb === "GET" || READS.has(path)) && !(body instanceof Blob);
	if (!reading) gets.clear();
	const reply = reading
		? await read(path, verb, body)
		: await send(path, verb, json ? JSON.stringify(body) : (body as Blob | undefined), json);
	if (!reading) gets.clear();
	if (reply.status === 204) return undefined as T;
	const data = reply.body as Record<string, unknown> | null;
	if (reply.status < 200 || reply.status > 299) {
		// the plausible style query errors start with a json pointer like `#/filters: `
		const message = String(data?.error ?? "Something went wrong.").replace(/^#\/\S*: /, "");
		throw new ApiError(reply.status, message, data ?? {});
	}
	return data as T;
}
