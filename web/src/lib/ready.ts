// holds a navigation until its first data is in, so the page you leave stays on screen
export async function ready(...asks: Promise<unknown>[]) {
	await Promise.race([Promise.allSettled(asks), new Promise((done) => setTimeout(done, 3000))]);
}
