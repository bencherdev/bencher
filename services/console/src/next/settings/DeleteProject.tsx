import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import TextInput from "@bencherdev/ui/TextInput";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createSignal, useContext } from "solid-js";
import type { JsonProject } from "../../types/bencher";
import { FlushContext } from "../flush";
import { useProject } from "../project";
import { forgetSlug } from "./data";
import { failureOf } from "./general";

/**
 * Deleting cannot be undone, so it waits for the reader to type the slug,
 * exactly; then the reader lands on the organization's projects.
 */
const DeleteProject = (props: {
	project: JsonProject;
	/** The organization's UUID, whose projects page is where the reader lands. */
	organization: string;
	onDismiss: () => void;
	/** Where the browser goes once the project is gone. */
	leave?: (href: string) => void;
}) => {
	const { api } = useProject();
	const client = useQueryClient();
	const flush = useContext(FlushContext);
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
		forgetSlug(client, localStorage, props.project.slug);
		// Leaving the app first would keep the project in the browser's store.
		await flush();
		(props.leave ?? ((href) => location.assign(href)))(
			`/console/organizations/${props.organization}/projects`,
		);
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
				<Button variant="ghost" aria-label="Close" onClick={props.onDismiss}>
					<Icon name="close" />
				</Button>
			</div>
			<div class="ui-dialog-body">
				<p id="del-desc" class="text2">
					Every report, threshold, alert, and pinned plot in{" "}
					{props.project.name} goes with it. Members lose access at once. It
					cannot be undone.
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
						aria-describedby="del-hint"
					/>
					<span class="fhelp" id="del-hint">
						{matches()
							? "The slug matches."
							: "Delete project stays off until the slug matches."}
					</span>
				</div>
				<Show when={failed()}>
					{(failure) => (
						<p class="ferror" role="alert">
							{failureOf(
								failure().error,
								"Bencher did not delete the project",
								"The Bencher API did not answer, so the project is still there.",
							)}
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
