import {
	is_valid_jwt,
	is_valid_user_name,
	is_valid_slug,
	is_valid_email,
	is_valid_plan_level,
} from "bencher_valid";
import type { JsonAuthUser } from "../types/bencher";

export const validOptionString = (
	input: undefined | null | string,
	validator: (input: string) => boolean,
): boolean => {
	if (!input) {
		return false;
	}
	return validString(input, validator);
};

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

export const validUuid = (uuid: string): boolean =>
	validString(uuid, (_uuid) => true);

export const validUserName = (user_name: string): boolean =>
	validString(user_name, is_valid_user_name);

export const validSlug = (slug: string): boolean =>
	validString(slug, is_valid_slug);

export const validEmail = (email: string): boolean =>
	validString(email, is_valid_email);

export const validJwt = (token: undefined | null | string): boolean =>
	validOptionString(token, is_valid_jwt);

export const validUser = (user: JsonAuthUser): boolean =>
	validUuid(user.user.uuid) &&
	validUserName(user.user.name) &&
	validSlug(user.user.slug) &&
	validEmail(user.user.email) &&
	validJwt(user.token);

export const validPlanLevel = (planLevel: undefined | null | string): boolean =>
	validOptionString(planLevel, is_valid_plan_level);
