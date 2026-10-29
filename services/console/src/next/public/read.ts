import { MAX_URL_LENGTH, URI_TOO_LONG } from "../url";

/** A failed read: the status the API answered with, if it answered. */
export class Refusal extends Error {
	readonly status: number | undefined;

	constructor(status: number | undefined) {
		super(`The API answered ${status ?? "nothing"}`);
		this.status = status;
	}
}

/** Reads the API at `apiUrl`, as `token`'s reader or as no one; a failure is a `Refusal`. */
export const reads =
	(apiUrl: string, token?: string) =>
	async <T,>(path: string): Promise<T> => {
		const target = `${apiUrl}${path}`;
		// An edge's refusal carries no CORS headers, so the page would read it as no network.
		if (target.length > MAX_URL_LENGTH) {
			throw new Refusal(URI_TOO_LONG);
		}
		let response: Response;
		try {
			response = await fetch(target, {
				headers: token ? { Authorization: `Bearer ${token}` } : {},
			});
		} catch {
			throw new Refusal(undefined);
		}
		if (!response.ok) {
			throw new Refusal(response.status);
		}
		return response.json();
	};
