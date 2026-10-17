import { type RouteSectionProps, useLocation } from "@solidjs/router";
import { Suspense, createEffect } from "solid-js";
import type { JsonAuthUser, JsonUser } from "../../types/bencher";
import type { Api } from "../api";
import ContentSkeleton from "../ContentSkeleton";
import { profilePath } from "../paths";
import Announcer from "../shell/Announcer";
import Shell from "../shell/Shell";
import { AccountContext } from "./context";

const TITLE = "Profile | Bencher";

/** The account bar and the page under it, for the reader's account paths. */
const AccountLayout = (
	props: RouteSectionProps & {
		api: Api;
		reader: () => JsonAuthUser;
		keepUser: (user: JsonUser) => void;
		signOut: () => void;
	},
) => {
	const location = useLocation();
	createEffect(() => {
		document.title = TITLE;
	});
	return (
		<AccountContext.Provider
			value={{
				api: props.api,
				reader: props.reader,
				keepUser: props.keepUser,
				signOut: props.signOut,
			}}
		>
			<Shell
				account={profilePath(props.reader().user.slug)}
				email={props.reader().user.email}
			/>
			<Announcer path={location.pathname} title={TITLE} />
			<Suspense fallback={<ContentSkeleton />}>{props.children}</Suspense>
		</AccountContext.Provider>
	);
};

export default AccountLayout;
