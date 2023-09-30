export const dateTimeMillis = (date_str: undefined | string) => {
	if (date_str === undefined) {
		return null;
	}
	const date_ms = Date.parse(date_str);
	if (date_ms) {
		const date = new Date(date_ms);
		if (date) {
			return date.getTime();
		}
	}
	return null;
};
