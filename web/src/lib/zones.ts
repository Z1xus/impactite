// chrome leaves utc out of the list
export const ZONES = [
	"UTC",
	...Intl.supportedValuesOf("timeZone").filter((zone) => zone !== "UTC")
].map((zone) => ({ key: zone, label: zone }));
