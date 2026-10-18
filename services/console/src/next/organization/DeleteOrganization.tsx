import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import TextInput from "@bencherdev/ui/TextInput";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createSignal, useContext } from "solid-js";
import type { JsonConsoleUser } from "../../types/bencher";
import { useAccount } from "../account/context";
import { FlushContext } from "../flush";
import { CLASSIC_ORGANIZATION, forgetVersion } from "../memory";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import { withOrganization } from "./data";
import { failureOf, goneWith } from "./general";

/**
 * Deleting cannot be undone, so it waits for the reader to type the slug,
 * exactly; then the reader lands on their organizations.
 */
const DeleteOrganization = (props: {
	organization: { uuid: string; slug: string };
	onDismiss: () => void;
	/** Where the browser goes once the organization is gone. */
	leave?: (href: string) => void;
}) => {
	const { api, reader } = useAccount();
	const client = useQueryClient();
	const flush = useContext(FlushContext);
	const user = useQueryResult(() => consoleUserQuery(api, reader().user.uuid));
	const projects = () =>
		user().data?.projects.filter(
			({ organization }) => organization === props.organization.uuid,
		) ?? [];
	const clause = () => goneWith(projects().map(({ name }) => name));
	const [typed, setTyped] = createSignal("");
	const [deleting, setDeleting] = createSignal(false);
	const [failed, setFailed] = createSignal<{ error: unknown }>();
	const remove = async () => {
		const { uuid, slug } = props.organization;
		const gone = projects().map((project) => project.slug);
		setDeleting(true);
		setFailed(undefined);
		try {
			await api.send("DELETE", `/v0/organizations/${uuid}`);
		} catch (error) {
			setFailed({ error });
			setDeleting(false);
			return;
		}
		for (const project of gone) {
			forgetVersion(localStorage, project);
		}
		client.removeQueries({
			predicate: ({ queryKey: [scope, key, project] }) =>
				(scope === "organization" && (key === uuid || key === slug)) ||
				(scope === "console" && gone.includes(project as string)),
		});
		client.setQueryData<JsonConsoleUser>(
			consoleUserQuery(api, reader().user.uuid).queryKey,
			(current) => current && withOrganization(current, uuid, undefined),
		);
		localStorage.removeItem(CLASSIC_ORGANIZATION);
		// Leaving the app first would keep the organization in the browser's store.
		await flush();
		(props.leave ?? ((href) => location.assign(href)))(
			"/console/organizations",
		);
	};
	return (
		<Dialog
			role="alertdialog"
			aria-labelledby="del-title"
			aria-describedby={clause() ? "del-desc" : undefined}
			onDismiss={props.onDismiss}
		>
			<div class="ui-dialog-head">
				<h2 id="del-title">Delete {props.organization.slug}?</h2>
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
				<Show when={clause()}>
					{(text) => (
						<p id="del-desc" class="text2">
							{text()}
						</p>
					)}
				</Show>
				<div class="field">
					<label for="del-slug" class="text2 sm">
						Type <b class="mono">{props.organization.slug}</b> to confirm
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
							{failureOf(
								failure().error,
								"Bencher did not delete the organization",
							)}
						</p>
					)}
				</Show>
			</div>
			<div class="ui-dialog-foot">
				<Button onClick={props.onDismiss}>Cancel</Button>
				<Button
					variant="destructive"
					disabled={typed() !== props.organization.slug || deleting()}
					onClick={remove}
				>
					Delete organization
				</Button>
			</div>
		</Dialog>
	);
};

export default DeleteOrganization;
