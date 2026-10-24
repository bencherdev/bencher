import { CONSOLE_PATH, USERS_PATH, type PageName } from "../paths";

/** The account's pages, by their path under the console without the user's slug. */
const ACCOUNT_PAGES = {
	"users/settings": { page: "Profile", title: "Profile" },
	"users/keys": { page: "UserKeys", title: "Keys" },
	"users/apps": { page: "ConnectedApps", title: "Connected Apps" },
	"users/tokens": { page: "LegacyTokens", title: "Legacy tokens" },
	organizations: { page: "Organizations", title: "Organizations" },
	"organizations/add": { page: "Organizations", title: "Organizations" },
} as const satisfies Record<string, { page: PageName; title: string }>;

const SHORTCUT = "users/apps";

type AccountPath = keyof typeof ACCOUNT_PAGES;
type UserSegment<P> = P extends `users/${infer S}` ? S : never;

export const accountPath = (user: string, segment: UserSegment<AccountPath>) =>
	`${USERS_PATH}/${user}/${segment}`;

/** The reader's own page a classic shortcut opens: the shortcut names no user, so the browser finds them. */
export const shortcutPage = (
	pathname: string,
): UserSegment<AccountPath> | undefined => {
	const [root, users, page, form, ...more] = pathname
		.split("/")
		.filter((part) => part.length > 0);
	if (`/${root}/${users}` !== USERS_PATH || more.length > 0) {
		return undefined;
	}
	if (page === undefined || page === "settings") {
		return form === undefined ? "settings" : undefined;
	}
	return (page === "keys" || page === "tokens") &&
		(form === undefined || form === "add")
		? page
		: undefined;
};

/** The account page `pathname` draws, with its title, if it is one. */
export const accountPage = (pathname: string) => {
	if (!pathname.startsWith(`${CONSOLE_PATH}/`)) {
		return undefined;
	}
	const parts = pathname
		.slice(CONSOLE_PATH.length + 1)
		.split("/")
		.filter((part) => part.length > 0);
	// The shortcut names no user: it is the reader's own page.
	if (parts[0] === "users" && parts.join("/") !== SHORTCUT) {
		parts.splice(1, 1);
	}
	const path = parts.join("/");
	return Object.hasOwn(ACCOUNT_PAGES, path)
		? ACCOUNT_PAGES[path as AccountPath]
		: undefined;
};
