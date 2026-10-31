/** A parameter value as the console shows it, with the empty string as `""` so it never reads as no value. */
export const shownValue = (value: string | number | boolean): string =>
	value === "" ? '""' : String(value);

/** A parameter as its `key=value` tag. */
export const parameterTag = (
	key: string,
	value: string | number | boolean,
): string => `${key}=${shownValue(value)}`;
