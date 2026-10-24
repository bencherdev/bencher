import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import TextInput from "@bencherdev/ui/TextInput";
import { useNavigate } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createSignal, useContext } from "solid-js";
import type { JsonConsoleUser, JsonProject } from "../../types/bencher";
import { AccountContext } from "../account/context";
import { FlushContext } from "../flush";
import { rememberedHome } from "../home";
import { CONSOLE_PATH } from "../paths";
import { useProject } from "../project";
import { consoleUserQuery } from "../queries";
import { forgetSlug, newestHome } from "./data";
import { failureOf } from "./general";

/**
 * Deleting cannot be undone, so it waits for the reader to type the slug,
 * exactly; then the reader lands on their main project, or on Organizations
 * with none left.
 */
const DeleteProject = (props: {
	project: JsonProject;
	onDismiss: () => void;
	/** Where the browser goes once the project is gone. */
	leave?: (href: string) => void;
}) => {
	const { api } = useProject();
	const client = useQueryClient();
	const flush = useContext(FlushContext);
	const navigate = useNavigate();
	const reader = useContext(AccountContext)?.reader().user.uuid;
	const [typed, setTyped] = createSignal("");
	const matches = () => typed() === props.project.slug;
	const [deleting, setDeleting] = createSignal(false);
	const [failed, setFailed] = createSignal<{ error: unknown }>();
	const remove = async () => {
		setDeleting(true);
		setFailed(undefined);
		try {
			await api.send(
				"DELETE",
				`/v0/projects/${encodeURIComponent(props.project.slug)}`,
			);
		} catch (error) {
			setFailed({ error });
			setDeleting(false);
			return;
		}
		const { slug } = props.project;
		forgetSlug(client, localStorage, slug);
		const projects = reader ? consoleUserQuery(api, reader) : undefined;
		if (projects) {
			client.setQueryData<JsonConsoleUser>(
				projects.queryKey,
				(user) =>
					user && {
						...user,
						projects: user.projects.filter((project) => project.slug !== slug),
					},
			);
		}
		// Before any full load, so the browser's store drops the project too.
		await flush();
		const to =
			rememberedHome(localStorage) ??
			newestHome(
				(projects &&
					(await client.fetchQuery(projects).catch(() => undefined))
						?.projects) ??
					[],
			);
		if (props.leave) {
			props.leave(to);
		} else if (to.startsWith(`${CONSOLE_PATH}/`)) {
			navigate(to.slice(CONSOLE_PATH.length));
		} else {
			location.assign(to);
		}
	};
	return (
		<Dialog
			role="alertdialog"
			aria-labelledby="del-title"
			aria-describedby="del-desc"
			onDismiss={props.onDismiss}
		>
			<div class="ui-dialog-head">
				<h2 id="del-title">Delete {props.project.slug}?</h2>
				<Button
					variant="ghost"
					iconOnly
					aria-label="Close"
					onClick={props.onDismiss}
				>
					<Icon name="close" />
				</Button>
			</div>
			<div class="ui-dialog-body">
				<p id="del-desc" class="text2">
					Every report, threshold, alert, and pinned plot goes with it, for
					good.
				</p>
				<div class="field">
					<label for="del-slug" class="text2 sm">
						Type <b class="mono">{props.project.slug}</b> to confirm
					</label>
					<TextInput
						id="del-slug"
						class="mono"
						value={typed()}
						onInput={(event) => setTyped(event.currentTarget.value)}
						autocomplete="off"
						spellcheck={false}
						autofocus
					/>
				</div>
				<Show when={failed()}>
					{(failure) => (
						<p class="ferror" role="alert">
							{failureOf(failure().error, "Bencher did not delete the project")}
						</p>
					)}
				</Show>
			</div>
			<div class="ui-dialog-foot">
				<Button onClick={props.onDismiss}>Cancel</Button>
				<Button
					variant="destructive"
					disabled={!matches() || deleting()}
					onClick={remove}
				>
					Delete project
				</Button>
			</div>
		</Dialog>
	);
};

export default DeleteProject;
