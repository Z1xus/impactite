import heart from "pixelarticons/svg/heart.svg?raw";
import server from "pixelarticons/svg/server.svg?raw";
import { icon } from "#lib/icons/resolve.ts";
import { hostOf } from "./format.ts";
import type { Monitor } from "./types.ts";

export const glyph = (monitor: Pick<Monitor, "kind" | "site" | "target">) =>
	monitor.kind === "heartbeat"
		? ({ kind: "pixel", svg: heart } as const)
		: monitor.kind === "tcp"
			? ({ kind: "pixel", svg: server } as const)
			: icon("event:hostname", monitor.site ?? hostOf(monitor.target));
