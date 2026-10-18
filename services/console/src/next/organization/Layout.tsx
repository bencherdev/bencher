import { useLocation } from "@solidjs/router";
import { type JSX, Suspense, createEffect } from "solid-js";
import { useAccount } from "../account/context";
import ContentSkeleton from "../ContentSkeleton";
import { organizationOf } from "../paths";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import Announcer from "../shell/Announcer";
import Shell from "../shell/Shell";
import { organizationIn, organizationQuery } from "./data";

/** The organization's bar and the page under it, for an organization's paths. */
const OrganizationLayout = (props: { children?: JSX.Element }) => {
	const { api, reader, signOut } = useAccount();
	const location = useLocation();
	const key = () => organizationOf(location.pathname) ?? "";
	const organization = useQueryResult(() => organizationQuery(api, key()));
	// Only read: the switcher asks for it on intent.
	const bootstrap = useQueryResult(() => ({
		...consoleUserQuery(api, reader().user.uuid),
		enabled: false,
	}));
	const known = () =>
		organization().data ?? organizationIn(bootstrap().data, key());
	const title = () =>
		`${location.pathname.endsWith("/projects/add") ? "New project" : "General"} | ${known()?.name ?? key()} | Bencher`;
	createEffect(() => {
		document.title = title();
	});
	return (
		<>
			<Shell
				org
				organization={known()}
				reader={reader().user.uuid}
				user={reader().user}
				signOut={signOut}
			/>
			<Announcer path={location.pathname} title={title()} />
			<Suspense fallback={<ContentSkeleton />}>{props.children}</Suspense>
		</>
	);
};

export default OrganizationLayout;
