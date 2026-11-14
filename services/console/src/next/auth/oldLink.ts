/** The address an old sign-in link's token names, read but not verified: only the API trusts it. */
export const tokenEmail = (token: string | undefined): string | undefined => {
	const payload = token?.split(".")[1];
	if (!payload) {
		return undefined;
	}
	try {
		const base64 = payload.replace(/-/g, "+").replace(/_/g, "/");
		const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
		const sub: unknown = JSON.parse(new TextDecoder().decode(bytes))?.sub;
		return typeof sub === "string" && sub ? sub : undefined;
	} catch {
		return undefined;
	}
};
