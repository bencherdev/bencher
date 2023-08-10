import {
	is_valid_jwt,
	is_valid_user_name,
	is_valid_slug,
	is_valid_email,
	is_valid_plan_level,
	is_valid_non_empty,
	is_valid_url,
	is_valid_card_number,
	is_valid_expiration_month,
	is_valid_expiration_year,
	is_valid_card_cvc,
} from "bencher_valid";
import type { JsonAuthUser } from "../types/bencher";

export const validOptionString = (
	input: undefined | null | string,
	validator: (input: string) => boolean,
): boolean => {
	if (typeof input !== "string") {
		return false;
	}
	return validString(input, validator);
};

export const validString = (
	input: string,
	validator: (input: string) => boolean,
): boolean => validator(input.trim());

export const validUuid = (uuid: string): boolean =>
	validString(uuid, (_uuid) => true);

export const validUserName = (user_name: string): boolean =>
	validString(user_name, is_valid_user_name);

export const validNonEmpty = (non_empty: string): boolean =>
	validString(non_empty, is_valid_non_empty);

export const validSlug = (slug: string): boolean =>
	validString(slug, is_valid_slug);

export const validEmail = (email: undefined | null | string): boolean =>
	validOptionString(email, is_valid_email);

export const validJwt = (token: undefined | null | string): boolean =>
	validOptionString(token, is_valid_jwt);

export const validOptionUrl = (url: undefined | null | string): boolean =>
	validOptionString(url, (i) => i.length === 0 || is_valid_url(i));

export const validUser = (user: JsonAuthUser): boolean =>
	validUuid(user.user.uuid) &&
	validUserName(user.user.name) &&
	validSlug(user.user.slug) &&
	validEmail(user.user.email) &&
	validJwt(user.token);

export const validU32 = (input: undefined | number | string) => {
	if (!input) {
		return false;
	}
	if (typeof input === "string" && input.length === 0) {
		return false;
	}
	const num = Number(input);
	return Number.isInteger(num) && num >= 0 && num <= 4_294_967_295;
};

export const validPlanLevel = (planLevel: undefined | null | string): boolean =>
	validOptionString(planLevel, is_valid_plan_level);

export const validCardNumber = (card_number: string): boolean => {
	return validString(card_number, (card_number) => {
		const number = cleanCardNumber(card_number);
		return is_valid_card_number(number);
	});
};

export const cleanCardNumber = (card_number: string): string => {
	return card_number
		.replace(new RegExp(" ", "g"), "")
		.replace(new RegExp("-", "g"), "");
};

export const validExpiration = (expiration: string): boolean => {
	return validString(expiration, (expiration) => {
		if (!/^\d{1,2}\/\d{2,4}$/.test(expiration)) {
			return false;
		}

		if (cleanExpiration(expiration) === null) {
			return false;
		} else {
			return true;
		}
	});
};

export const cleanExpiration = (expiration: string): null | number[] => {
	const exp = expiration.split("/");
	if (exp.length !== 2) {
		return null;
	}

	const month = exp?.[0];
	if (month === undefined) {
		return null;
	}
	const exp_month = Number.parseInt(month);
	if (Number.isInteger(exp_month)) {
		if (!is_valid_expiration_month(exp_month)) {
			return null;
		}
	} else {
		return null;
	}

	const year = exp?.[1];
	if (year === undefined) {
		return null;
	}
	let exp_year = Number.parseInt(year);
	if (Number.isInteger(exp_year)) {
		if (exp_year < 100) {
			exp_year = 2000 + exp_year;
		}
		if (!is_valid_expiration_year(exp_year)) {
			return null;
		}
	} else {
		return null;
	}

	return [exp_month, exp_year];
};

export const validCardCvc = (card_cvc: string): boolean => {
	return validString(card_cvc, is_valid_card_cvc);
};
