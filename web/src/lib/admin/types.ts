export type User = {
	id: number;
	name: string;
	email: string;
	admin: boolean;
	created: number;
	sites: number;
};
export type Admin = {
	signup: boolean;
	mail: boolean;
	users: User[];
	version: string;
	update: {
		enabled: boolean;
		checking: boolean;
		checked_at: number | null;
		failed: boolean;
		latest: {
			version: string;
			url: string;
			changes: { version: string; lines: string[] }[];
		} | null;
	};
};
