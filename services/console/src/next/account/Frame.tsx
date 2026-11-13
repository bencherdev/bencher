import { For, type JSX } from "solid-js";
import { ORGANIZATIONS_PATH, SERVER_USERS_PATH, profilePath } from "../paths";
import { useAccount } from "./context";
import { accountPath } from "./paths";

type Section = "profile" | "keys" | "apps" | "organizations" | "users";

/** The account's sections, as Settings' rail, and the server's users for a server admin. */
const Frame = (props: {
	user: string;
	section: Section;
	children: JSX.Element;
}) => {
	const { reader } = useAccount();
	const sections = (): { section: Section; label: string; href: string }[] => [
		{ section: "profile", label: "Profile", href: profilePath(props.user) },
		{ section: "keys", label: "Keys", href: accountPath(props.user, "keys") },
		{
			section: "apps",
			label: "Connected Apps",
			href: accountPath(props.user, "apps"),
		},
		{
			section: "organizations",
			label: "Organizations",
			href: ORGANIZATIONS_PATH,
		},
		...(reader().admin
			? [{ section: "users" as const, label: "Users", href: SERVER_USERS_PATH }]
			: []),
	];
	const links = () => (
		<For each={sections()}>
			{({ section, label, href }) => (
				<a
					href={href}
					aria-current={section === props.section ? "page" : undefined}
				>
					{label}
				</a>
			)}
		</For>
	);
	return (
		<main class="page setwrap">
			<nav class="rail" aria-label="Account">
				<span class="eyebrow">Account</span>
				{links()}
			</nav>
			<nav class="railseg" aria-label="Account sections">
				{links()}
			</nav>
			<div class="setbody">{props.children}</div>
		</main>
	);
};

export default Frame;
