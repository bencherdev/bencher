import { useLocation } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import {
	Match,
	Show,
	Switch,
	createComputed,
	createMemo,
	lazy,
} from "solid-js";
import ContentSkeleton from "../ContentSkeleton";
import { parseConsolePath } from "../paths";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { keysQuery, projectKeys } from "../settings/data";
import Frame from "../settings/Frame";
import General from "../settings/General";
import Keys from "../settings/Keys";
import { sectionOf } from "../settings/section";

// A static import would leave the account's key pages, which load this chunk,
// reading NotFound through a namespace whose helper sits in a classic chunk.
const NotFound = lazy(() => import("./NotFound"));

/** General and Keys, with Dimensions beside them in the rail. */
const Settings = () => {
	const location = useLocation();
	const { api, slug } = useProject();
	const client = useQueryClient();
	const section = createMemo(() =>
		sectionOf(parseConsolePath(location.pathname)?.rest ?? []),
	);
	// A section paints once the shell's answer says what the reader may do.
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	// The key lists start beside the shell's request, not a round after it.
	createComputed(() => {
		if (
			section() === "keys" &&
			bootstrap().data?.permissions.manage !== false
		) {
			client.prefetchQuery(keysQuery(api, projectKeys(slug()), false));
			client.prefetchQuery(keysQuery(api, projectKeys(slug()), true));
		}
	});
	return (
		<Show when={bootstrap().data} fallback={<ContentSkeleton />}>
			<Switch fallback={<NotFound />}>
				<Match when={section() === "general"}>
					<Frame slug={slug()} section="general">
						<General />
					</Frame>
				</Match>
				<Match when={section() === "keys"}>
					<Frame slug={slug()} section="keys">
						<Keys />
					</Frame>
				</Match>
			</Switch>
		</Show>
	);
};

export default Settings;

// The account's key pages load this chunk rather than split it.
export { keysKey, keysQuery, revokeKey } from "../settings/data";
export { failureOf } from "../settings/failure";
export { KeyTable, default as KeyList, Revoke } from "../settings/KeyList";
export { type KeyRow, newestFirst } from "../settings/keys";
