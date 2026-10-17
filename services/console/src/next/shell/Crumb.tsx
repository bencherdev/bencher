import Icon from "@bencherdev/ui/Icon";
import { QueryClientContext } from "@tanstack/solid-query";
import { Show, createSignal, useContext } from "solid-js";
import { ProjectContext } from "../project";
import { consoleUserQuery } from "../queries";

type View = typeof import("../switcher/Switcher").default;

let loaded: View | undefined;
let loading: Promise<View> | undefined;
const load = () => {
	loading ??= import("../switcher/Switcher").then(
		({ default: view }) => {
			loaded = view;
			return view;
		},
		(error: unknown) => {
			loading = undefined;
			throw error;
		},
	);
	return loading;
};

/** The Org / Project crumb, one button that opens the project switcher. */
const Crumb = (props: {
	slug: string;
	organization: { name: string; uuid: string } | undefined;
	project: string;
	/** The signed in reader, unknown to the server's render. */
	reader?: string | undefined;
}) => {
	const client = useContext(QueryClientContext);
	const project = useContext(ProjectContext);
	const [view, setView] = createSignal(loaded);
	const [open, setOpen] = createSignal(false);
	let button: HTMLButtonElement | undefined;
	// The switcher's code and the reader's projects start together on intent.
	const intent = () => {
		load().catch(() => {});
		if (client && project && props.reader) {
			client()
				.query(consoleUserQuery(project.api, props.reader))
				.catch(() => {});
		}
	};
	return (
		<div class="crumb">
			<button
				ref={button}
				type="button"
				class="crumbbtn"
				aria-haspopup="dialog"
				aria-expanded={open()}
				aria-label={`${props.organization ? `${props.organization.name} / ` : ""}${props.project}, switch project`}
				aria-busy={props.organization ? undefined : "true"}
				onPointerEnter={intent}
				onFocus={intent}
				onKeyDown={(event) => {
					if (event.key === "Escape" && open()) {
						setOpen(false);
					}
				}}
				onClick={() => {
					if (open()) {
						setOpen(false);
						return;
					}
					load()
						.then((view) => {
							setView(() => view);
							setOpen(true);
						})
						.catch(() => {});
				}}
			>
				<Show
					when={props.organization}
					fallback={
						<span
							class="ui-skeleton crumb-skel"
							data-size="text"
							data-shell="org"
							aria-hidden="true"
						/>
					}
				>
					{(organization) => (
						<span data-shell="org">{organization().name}</span>
					)}
				</Show>{" "}
				<span class="sep" aria-hidden="true">
					/
				</span>{" "}
				<span class="crumb-project" data-shell="project">
					{props.project}
				</span>
				<Icon name="chevron-down" />
			</button>
			<Show when={open() && props.reader ? view() : undefined}>
				{(view) => {
					const Switcher = view();
					return (
						<Switcher
							reader={props.reader ?? ""}
							slug={props.slug}
							organization={props.organization}
							crumb={button}
							onClose={(refocus) => {
								setOpen(false);
								if (refocus) {
									button?.focus();
								}
							}}
						/>
					);
				}}
			</Show>
		</div>
	);
};

export default Crumb;
