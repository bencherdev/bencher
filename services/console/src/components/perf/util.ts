export const fetchSSR = async (url: string, timeout = 1_000) =>
	await fetch(url, { signal: AbortSignal.timeout(timeout) });
