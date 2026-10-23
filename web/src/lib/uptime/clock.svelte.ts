export const tick = $state({ now: Math.floor(Date.now() / 1000) });

setInterval(() => (tick.now = Math.floor(Date.now() / 1000)), 10_000);
