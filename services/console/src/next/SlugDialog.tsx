import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";

/** Confirms a new slug for an organization or a project before it is sent. */
const SlugDialog = (props: {
	slug: string;
	noun: "organization" | "project";
	onCancel: () => void;
	onConfirm: () => void;
}) => (
	<Dialog
		role="alertdialog"
		aria-labelledby="slug-title"
		aria-describedby="slug-desc"
		onDismiss={props.onCancel}
	>
		<div class="ui-dialog-head">
			<h2 id="slug-title">Change the slug to {props.slug}?</h2>
			<Button
				variant="ghost"
				iconOnly
				aria-label="Close"
				onClick={props.onCancel}
			>
				<Icon name="close" />
			</Button>
		</div>
		<div class="ui-dialog-body">
			<p id="slug-desc">Every existing link to this {props.noun} breaks.</p>
		</div>
		<div class="ui-dialog-foot">
			<Button onClick={props.onCancel}>Cancel</Button>
			<Button variant="destructive" onClick={props.onConfirm}>
				Change slug
			</Button>
		</div>
	</Dialog>
);

export default SlugDialog;
