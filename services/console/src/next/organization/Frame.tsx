import { For, type JSX } from "solid-js";
import { ORGANIZATIONS_PATH, organizationPath } from "../paths";

type Section = "general" | "members" | "billing";

/** The organization's sections, as the account's rail. */
const Frame = (props: {
	/** The organization's slug or UUID, as the path has it. */
	organization: string;
	/** Its name once known. */
	name: string | undefined;
	section: Section;
	children: JSX.Element;
}) => {
	const sections = (): { section: Section; label: string; href: string }[] => [
		{
			section: "general",
			label: "General",
			href: organizationPath(props.organization),
		},
		{
			section: "members",
			label: "Members",
			href: `${ORGANIZATIONS_PATH}/${props.organization}/members`,
		},
		{
			section: "billing",
			label: "Billing",
			href: `${ORGANIZATIONS_PATH}/${props.organization}/billing`,
		},
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
			<nav class="rail" aria-label="Organization settings">
				<span class="eyebrow">{props.name ?? props.organization}</span>
				{links()}
			</nav>
			<nav class="railseg" aria-label="Organization settings sections">
				{links()}
			</nav>
			<div class="setbody">{props.children}</div>
		</main>
	);
};

export default Frame;
