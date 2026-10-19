import { get } from "#lib/api.ts";
import type { Admin } from "./types.ts";

export const admin = $state<{ info: Admin | null; failed: boolean }>({ info: null, failed: false });

export async function load() {
	admin.failed = false;
	try {
		admin.info = await get<Admin>("/api/app/admin");
	} catch (err) {
		if (admin.info) throw err;
		admin.failed = true;
	}
}
