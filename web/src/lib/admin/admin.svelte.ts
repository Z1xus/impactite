import { api } from "#lib/api.ts";
import type { Admin } from "./types.ts";

export const admin = $state<{ info: Admin | null }>({ info: null });

export async function load() {
	admin.info = await api<Admin>("/api/app/admin");
}
