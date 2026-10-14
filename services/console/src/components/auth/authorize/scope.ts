import type { OAuthScope } from "../../../types/bencher";

// Keyed by value: importing the enum regroups the shared types chunk and grows every new console page.
const SCOPE_TEXT: Record<OAuthScope, string> = {
	"projects:read":
		"View your projects and their results, including private projects",
	"projects:write":
		"View, create, and change data in your projects, including private projects, such as reports, thresholds, and alerts",
	openid: "Know which Bencher account you are signed in as",
	email: "View your email address and whether it is verified",
	profile: "View your name and username",
};

export const scopeText = (scope: OAuthScope): string =>
	SCOPE_TEXT[scope] ?? scope;
