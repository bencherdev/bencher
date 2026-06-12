import { Show } from "solid-js";
import type { JsonAuthUser } from "../../../types/bencher";

enum Section {
	KEYS = "keys",
	TOKENS = "tokens",
	SETTINGS = "settings",
	HELP = "help",
}

// User API tokens are deprecated:
// accounts created after this date never see the API Tokens page.
const API_TOKENS_SUNSET = new Date("2026-06-30T23:59:59Z");

const UserMenu = (props: { user?: JsonAuthUser }) => {
	const path = (section: Section) =>
		`/console/users/${props.user?.user?.slug}/${section}`;

	// Fail open: a missing or unparsable `created` date means the account
	// predates the sunset (or localStorage predates the field), so show the entry.
	const showTokens = () => {
		const created = props.user?.user?.created;
		return !created || !(new Date(created) > API_TOKENS_SUNSET);
	};

	return (
		<aside class="menu is-sticky">
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<a href="/console/organizations">Organizations</a>
				</li>
				<li>
					<a href={path(Section.KEYS)}>API Keys</a>
				</li>
				<Show when={showTokens()}>
					<li>
						<a href={path(Section.TOKENS)}>API Tokens</a>
					</li>
				</Show>
				<li>
					<a href={path(Section.SETTINGS)}>Settings</a>
				</li>
				<li>
					<a href={path(Section.HELP)}>Help</a>
				</li>
			</ul>
		</aside>
	);
};

export default UserMenu;
