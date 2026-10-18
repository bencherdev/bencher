import { useLocation } from "@solidjs/router";
import { type JSX, Suspense, createEffect } from "solid-js";
import ContentSkeleton from "../ContentSkeleton";
import Announcer from "../shell/Announcer";
import Shell from "../shell/Shell";
import { useAccount } from "./context";
import { accountPage } from "./paths";

/** The account bar and the page under it, for the reader's account paths. */
const AccountLayout = (props: { children?: JSX.Element }) => {
	const { reader, signOut } = useAccount();
	const location = useLocation();
	const title = () =>
		`${accountPage(location.pathname)?.title ?? "Account"} | Bencher`;
	createEffect(() => {
		document.title = title();
	});
	return (
		<>
			<Shell user={reader().user} signOut={signOut} />
			<Announcer path={location.pathname} title={title()} />
			<Suspense fallback={<ContentSkeleton />}>{props.children}</Suspense>
		</>
	);
};

export default AccountLayout;
