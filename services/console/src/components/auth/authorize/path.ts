/** The page where a user answers an app's request to connect. */
const CONSENT_PATH = "/auth/authorize";
export const REQUEST_PARAM = "request";
/** Carries an app's request to connect through sign in, back to its consent page. */
export const AUTHORIZATION_PARAM = "authorization";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The consent page for a request, the only place a sign in returns to with one. */
export const consentPath = (authorization: unknown): undefined | string => {
	const uuid = requestUuid(authorization);
	return uuid ? `${CONSENT_PATH}?${REQUEST_PARAM}=${uuid}` : undefined;
};

/** A request's UUID, or nothing for any other value. */
export const requestUuid = (value: unknown): undefined | string =>
	typeof value === "string" && UUID.test(value)
		? value.toLowerCase()
		: undefined;

/** `path`, returning to the request after sign in when `authorization` is one. */
export const signInPath = (path: string, authorization: unknown): string => {
	const uuid = requestUuid(authorization);
	return uuid ? `${path}?${AUTHORIZATION_PARAM}=${uuid}` : path;
};
