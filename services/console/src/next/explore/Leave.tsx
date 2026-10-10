import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";

/** Asked before leaving a pin with unsaved changes for another page. */
const Leave = (props: {
	title: string;
	/** The page the reader is leaving for, as its tab names it. */
	destination: string;
	saving: boolean;
	onStay: () => void;
	onLeave: () => void;
	onSave: () => void;
}) => (
	<Dialog
		role="alertdialog"
		aria-labelledby="ex-leave-title"
		aria-describedby="ex-leave-desc"
		onDismiss={props.onStay}
	>
		<div class="ui-dialog-head">
			<h2 id="ex-leave-title">Leave without saving?</h2>
			<Button variant="ghost" aria-label="Keep editing" onClick={props.onStay}>
				<Icon name="close" />
			</Button>
		</div>
		<div class="ui-dialog-body">
			<p class="ex-leave">
				<span class="ex-warn">
					<Icon name="warning" />
				</span>
				<span>
					<b>{props.title}</b> has changes since its last save.
				</span>
			</p>
			<p id="ex-leave-desc" class="text2">
				Save writes them to the pinned plot. Leaving for{" "}
				<b>{props.destination}</b> drops them; the pinned plot stays as it was
				saved.
			</p>
		</div>
		<div class="ui-dialog-foot">
			<Button onClick={props.onStay}>Keep editing</Button>
			<span class="spacer" />
			<Button onClick={props.onLeave}>Leave without saving</Button>
			<Button variant="primary" disabled={props.saving} onClick={props.onSave}>
				Save and leave
			</Button>
		</div>
	</Dialog>
);

export default Leave;
