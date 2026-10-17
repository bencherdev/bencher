import Icon from "@bencherdev/ui/Icon";
import { QueryClientContext } from "@tanstack/solid-query";
import { Show, createSignal, useContext } from "solid-js";
import { useApi } from "../project";
import { consoleUserQuery } from "../queries";
import { lazyView } from "./lazy";

const switcher = lazyView(() => import("../switcher/Switcher"));

/** The Org / Project crumb, one button that opens the project switcher; an organization's page has no project. */
const Crumb = (props: {
	slug: string;
	organization: { name: string; uuid: string } | undefined;
	project?: string | undefined;
	/** The signed in reader, unknown to the server's render. */
	reader?: string | undefined;
}) => {
	const client = useContext(QueryClientContext);
	const api = useApi();
	const [view, setView] = createSignal(switcher.loaded());
	const [open, setOpen] = createSignal(false);
	let button: HTMLButtonElement | undefined;
	// The switcher's code and the reader's projects start together on intent.
	const intent = () => {
		switcher.preload();
		if (client && api && props.reader) {
			client()
				.query(consoleUserQuery(api, props.reader))
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
				aria-label={`${[props.organization?.name, props.project].filter(Boolean).join(" / ")}, switch project`}
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
					switcher
						.load()
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
						<span
							class={props.project ? undefined : "crumb-project"}
							data-shell="org"
						>
							{organization().name}
						</span>
					)}
				</Show>
				<Show when={props.project}>
					{(project) => (
						<>
							{" "}
							<span class="sep" aria-hidden="true">
								/
							</span>{" "}
							<span class="crumb-project" data-shell="project">
								{project()}
							</span>
						</>
					)}
				</Show>
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
