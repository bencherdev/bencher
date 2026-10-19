/** What this tab signs in as, so only the login or signup link or provider callback for exactly that signs in unasked. */
const SIGN_IN_KEY = "BENCHER_SIGN_IN";

/** `value` is the email as the API keeps it, or the state a provider sign in carries. */
export const startSignIn = (value: string) =>
	sessionStorage.setItem(SIGN_IN_KEY, value);

/** Whether this tab started signing in as `value`, which it forgets either way. */
export const takeSignIn = (value: undefined | string): boolean => {
	const started = sessionStorage.getItem(SIGN_IN_KEY);
	sessionStorage.removeItem(SIGN_IN_KEY);
	return started === value;
};
