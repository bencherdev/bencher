import { ApiError } from "../api";

/** What did not happen, with what the API said when it refused. */
export const failureOf = (error: unknown, failed: string) =>
	error instanceof ApiError && error.status !== undefined
		? `${failed}: ${apiMessage(error)}`
		: failed;

export const apiMessage = (error: ApiError) => {
	try {
		const message = JSON.parse(error.message)?.message;
		if (typeof message === "string" && message) {
			return message;
		}
	} catch {
		// Not the API's JSON; its status says enough.
	}
	return `the API answered ${error.status}`;
};
