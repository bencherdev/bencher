import { useLocation } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { Match, Show, Switch, createComputed, createMemo } from "solid-js";
import ContentSkeleton from "../ContentSkeleton";
import { parseNextPath } from "../paths";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { keysQuery } from "../settings/data";
import Frame from "../settings/Frame";
import General from "../settings/General";
import Head from "../settings/Head";
import Keys from "../settings/Keys";
import { sectionOf } from "../settings/section";
import Placeholder from "./Placeholder";

/** General and Keys, with Dimensions beside them in the rail. */
const Settings = () => {
	const location = useLocation();
	const { api, slug } = useProject();
	const client = useQueryClient();
	const section = createMemo(() =>
		sectionOf(parseNextPath(location.pathname)?.rest ?? []),
	);
	// A section paints once the shell's answer says what the reader may do.
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	// The key lists start beside the shell's request, not a round after it.
	createComputed(() => {
		if (
			section() === "keys" &&
			bootstrap().data?.permissions.manage !== false
		) {
			client.prefetchQuery(keysQuery(api, slug(), false));
			client.prefetchQuery(keysQuery(api, slug(), true));
		}
	});
	return (
		<Show when={bootstrap().data} fallback={<ContentSkeleton />}>
			<Switch fallback={<Placeholder title="Page not found" />}>
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
				<Match when={section() === "dimensions"}>
					<Frame slug={slug()} section="dimensions">
						<Head slug={slug()} title="Dimensions" readOnly={false} />
					</Frame>
				</Match>
			</Switch>
		</Show>
	);
};

export default Settings;
