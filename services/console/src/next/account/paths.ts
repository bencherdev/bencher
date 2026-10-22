import { NEXT_CONSOLE, NEXT_USERS, type PageName } from "../paths";

/** The account's pages, by their path under the console without the user's slug. */
const ACCOUNT_PAGES = {
	"users/settings": { page: "Profile", title: "Profile" },
	"users/keys": { page: "UserKeys", title: "Keys" },
	"users/apps": { page: "ConnectedApps", title: "Connected Apps" },
	"users/tokens": { page: "LegacyTokens", title: "Legacy tokens" },
	organizations: { page: "Organizations", title: "Organizations" },
	"organizations/add": { page: "Organizations", title: "Organizations" },
	// GitHub's return from linking a repository, under the reader's account bar.
	github: { page: "GitHub", title: "GitHub" },
} as const satisfies Record<string, { page: PageName; title: string }>;

const SHORTCUT = "users/apps";

type AccountPath = keyof typeof ACCOUNT_PAGES;
type UserSegment<P> = P extends `users/${infer S}` ? S : never;

export const accountPath = (user: string, segment: UserSegment<AccountPath>) =>
	`${NEXT_USERS}/${user}/${segment}`;

/** The account page `pathname` draws, with its title, if it is one. */
export const accountPage = (pathname: string) => {
	if (!pathname.startsWith(`${NEXT_CONSOLE}/`)) {
		return undefined;
	}
	const parts = pathname
		.slice(NEXT_CONSOLE.length + 1)
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
