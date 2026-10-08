import type { JSX } from "solid-js";
import { createEffect, createUniqueId, splitProps } from "solid-js";
import Button from "./Button";

export interface SheetProps
	extends Omit<JSX.DialogHtmlAttributes<HTMLDialogElement>, "title"> {
	open: boolean;
	onClose: () => void;
	/** The heading, which also names the dialog. */
	title: string;
	/** The label of the button that closes it. */
	done?: string;
}

/**
 * A modal sheet that rises from the bottom of the screen: a native dialog, so
 * the page behind it is inert, the focus stays inside, and Escape closes it.
 * A press on the dimmed page closes it too.
 */
const Sheet = (props: SheetProps) => {
	const [local, rest] = splitProps(props, [
		"open",
		"onClose",
		"title",
		"done",
		"class",
		"children",
	]);
	const heading = createUniqueId();
	let dialog: HTMLDialogElement | undefined;
	createEffect(() => {
		if (!dialog) {
			return;
		}
		if (local.open && !dialog.open) {
			dialog.showModal();
		} else if (!local.open && dialog.open) {
			dialog.close();
		}
	});
	return (
		// biome-ignore lint/a11y/useKeyWithClickEvents: Escape closes a native dialog; the click is only on the dimmed page around it
		<dialog
			ref={dialog}
			class={local.class === undefined ? "ui-sheet" : `ui-sheet ${local.class}`}
			aria-labelledby={heading}
			onClose={() => local.onClose()}
			onClick={(event) => {
				if (event.target === dialog) {
					local.onClose();
				}
			}}
			{...rest}
		>
			<div class="ui-sheet-head">
				<h2 id={heading} class="ui-sheet-title">
					{local.title}
				</h2>
				<Button onClick={() => local.onClose()}>{local.done ?? "Done"}</Button>
			</div>
			<div class="ui-sheet-body">{local.children}</div>
		</dialog>
	);
};

export default Sheet;
