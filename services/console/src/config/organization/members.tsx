import { validEmail, validSlug, validUserName } from "../../util/valid";

const MEMBER_ICON = "fas fa-user";

export const MEMBER_FIELDS = {
	name: {
		type: "text",
		placeholder: "Member Name",
		icon: MEMBER_ICON,
		help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
		validate: validUserName,
	},
	slug: {
		type: "text",
		placeholder: "Member Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be at least four characters or longer",
		validate: validSlug,
	},
	email: {
		type: "email",
		placeholder: "email@example.com",
		icon: "fas fa-envelope",
		help: "Must be a valid email address",
		validate: validEmail,
	},
	role: {
		icon: "fas fa-user-tag",
	},
};
