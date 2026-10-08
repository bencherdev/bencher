import type { JSX } from "solid-js";
import { onCleanup, onMount, splitProps } from "solid-js";

export interface DialogProps
	extends JSX.DialogHtmlAttributes<HTMLDialogElement> {
	/**
	 * The reader asked to leave: Escape, or a control that calls it. Without
	 * it, Escape does nothing, for a dialog that must be answered.
	 */
	onDismiss?: (() => void) | undefined;
}

/**
 * A modal dialog, open while it is mounted. The browser traps focus inside it,
 * makes the page behind it inert, and returns focus to where it was when it
 * closes. Give it `aria-labelledby`, and `role="alertdialog"` when it asks to
 * confirm what cannot be undone. Compose it from `ui-dialog-head`,
 * `ui-dialog-body`, and `ui-dialog-foot`; on a narrow screen it is a sheet.
 */
const Dialog = (props: DialogProps) => {
	const [local, rest] = splitProps(props, ["class", "onDismiss"]);
	let dialog: HTMLDialogElement | undefined;
	let leaving = false;
	onMount(() => dialog?.showModal());
	// Closing before it leaves the page is what returns focus to the opener.
	onCleanup(() => {
		leaving = true;
		dialog?.close();
	});
	return (
		<dialog
			ref={(element) => {
				dialog = element;
			}}
			class={
				local.class === undefined ? "ui-dialog" : `ui-dialog ${local.class}`
			}
			onCancel={(event) => {
				event.preventDefault();
				local.onDismiss?.();
			}}
			// The browser can close it without asking, after repeated Escapes; one
			// that must be answered opens again.
			onClose={() => {
				if (leaving) {
					return;
				}
				if (local.onDismiss) {
					local.onDismiss();
				} else {
					dialog?.showModal();
				}
			}}
			{...rest}
		/>
	);
};

export default Dialog;
