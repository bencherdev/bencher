import { is_valid_jwt, is_valid_user_name, is_valid_email } from "bencher_valid";

export const validString = (
	input: string,
	validator: (input: string) => boolean,
): boolean => {
	if (typeof input === "string") {
		return validator(input.trim());
	} else {
		return false;
	}
};

export const validUserName = (user_name: string): boolean => {
	return validString(user_name, is_valid_user_name);
};

export const validEmail = (email: string): boolean => {
	return validString(email, is_valid_email);
};

export const validJwt = (token: undefined | null | string): boolean => {
	if (!token) {
		return false;
	}
	return validString(token, is_valid_jwt);
};