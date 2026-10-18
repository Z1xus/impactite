export type Member = { id: number; name: string; email: string; role: string };
export type Invite = { id: number; email: string; role: string; expires: number };
export type Team = {
	id: number;
	name: string;
	personal: boolean;
	members: Member[];
	invites: Invite[];
};
export type Role = "admin" | "editor" | "viewer";

export const ROLES: { key: Role; label: string }[] = [
	{ key: "admin", label: "admin" },
	{ key: "editor", label: "editor" },
	{ key: "viewer", label: "viewer" }
];
