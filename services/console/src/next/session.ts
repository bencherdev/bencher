/** The session this browser is signed in to, which only its HttpOnly cookie names. */
const SESSION_PATH = "/api/v0/auth/session";

/**
 * End the session this browser is signed in to. One that already ended is
 * refused, and ends all the same; an API out of reach keeps it until it ends
 * on its own.
 */
export const endSession = async (fetch = globalThis.fetch.bind(globalThis)) => {
	try {
		const response = await fetch(SESSION_PATH, { method: "DELETE" });
		// An unread `no-store` body holds its connection.
		await response.text();
	} catch {
		// The browser forgets the reader either way.
	}
};
