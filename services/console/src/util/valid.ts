export const validate_string = (
	input: string,
	validator: (input: string) => boolean,
): boolean => {
	if (typeof input === "string") {
		return validator(input.trim());
	} else {
		return false;
	}
};