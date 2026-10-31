import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import TextArea from "@bencherdev/ui/TextArea";
import Truncate from "@bencherdev/ui/Truncate";
import {
	For,
	Show,
	createSignal,
	createUniqueId,
	onCleanup,
	onMount,
} from "solid-js";
import { version } from "../../../package.json";
import { type Attached, type Kind, browserOf, sizeOf } from "./report";
import { send } from "./send";

const KINDS: { value: Kind; label: string }[] = [
	{ value: "bug", label: "Bug" },
	{ value: "feedback", label: "Feedback" },
];

/** How long Sent stays up before the dialog closes itself. */
const SENT_FOR = 3_000;

export interface FeedbackAbout {
	/** The organization and project as the crumb names them, on a project's page. */
	project?: string | undefined;
	slug?: string | undefined;
	email: string;
}

const FeedbackDialog = (props: {
	about: FeedbackAbout;
	onClose: () => void;
}) => {
	const heading = createUniqueId();
	const messageId = createUniqueId();
	const [kind, setKind] = createSignal<Kind>("bug");
	const [message, setMessage] = createSignal("");
	const [image, setImage] = createSignal<File>();
	const [off, setOff] = createSignal<ReadonlySet<Attached>>(new Set());
	const [phase, setPhase] = createSignal<
		"compose" | "sending" | "sent" | "failed"
	>("compose");
	const [copied, setCopied] = createSignal(false);
	let picker: HTMLInputElement | undefined;

	const items: { id: Attached; label: string; value: string }[] = [
		{
			id: "page",
			label: "Page",
			value: document.title.replace(/ \| Bencher$/, "").replaceAll(" | ", ", "),
		},
		...(props.about.project
			? [
					{
						id: "project" as const,
						label: "Project",
						value: props.about.project,
					},
				]
			: []),
		{ id: "browser", label: "Browser", value: browserOf(navigator.userAgent) },
		{ id: "build", label: "Console build", value: version },
		{ id: "session", label: "Session link", value: "Sentry trace and replay" },
		{ id: "email", label: "Your email", value: props.about.email },
	];
	const toggle = (id: Attached) => {
		const next = new Set(off());
		if (!next.delete(id)) {
			next.add(id);
		}
		setOff(next);
	};

	const submit = async () => {
		setPhase("sending");
		const file = image();
		try {
			await send(
				{
					kind: kind(),
					message: message(),
					image: file && {
						filename: file.name,
						contentType: file.type,
						data: new Uint8Array(await file.arrayBuffer()),
					},
					off: off(),
				},
				{
					url: location.href,
					slug: props.about.slug,
					email: props.about.email,
					build: version,
				},
			);
			setPhase("sent");
		} catch {
			setPhase("failed");
		}
	};

	return (
		<Dialog aria-labelledby={heading} onDismiss={props.onClose}>
			<div class="ui-dialog-head">
				<h2 id={heading} class="fb-title">
					<Icon name="feedback" />
					Send feedback
				</h2>
				<Button
					variant="ghost"
					iconOnly
					aria-label="Close"
					onClick={props.onClose}
				>
					<Icon name="close" />
				</Button>
			</div>
			<Show
				when={phase() !== "sent"}
				fallback={<Sent onClose={props.onClose} />}
			>
				<div class="ui-dialog-body">
					<Show when={phase() === "failed"}>
						<Banner status="error" role="alert" class="fb-failed">
							<Icon name="warning" />
							<div class="fb-failed-body">
								<b>Not sent: blocked by this browser</b>
								<div class="fb-failed-actions">
									<Button
										size="sm"
										onClick={() =>
											navigator.clipboard
												.writeText(message())
												.then(() => setCopied(true))
												.catch(() => {})
										}
									>
										{copied() ? "Copied" : "Copy text"}
									</Button>
									<ButtonLink size="sm" href="mailto:help@bencher.dev">
										Email help@bencher.dev
									</ButtonLink>
								</div>
							</div>
						</Banner>
					</Show>
					<Segmented
						name="feedback-kind"
						aria-label="Kind"
						class="fb-kind"
						options={KINDS}
						value={kind()}
						onChange={setKind}
					/>
					<div class="field">
						<label class="flab" for={messageId}>
							Message
						</label>
						<TextArea
							id={messageId}
							class="fb-message"
							value={message()}
							onInput={(event) => setMessage(event.currentTarget.value)}
						/>
					</div>
					<input
						ref={picker}
						type="file"
						accept="image/*"
						aria-label="Add an image"
						hidden
						onChange={(event) => {
							setImage(event.currentTarget.files?.[0]);
							event.currentTarget.value = "";
						}}
					/>
					<Show
						when={image()}
						fallback={
							<Button size="sm" class="fb-add" onClick={() => picker?.click()}>
								Add an image
							</Button>
						}
					>
						{(file) => (
							<div class="fb-attach">
								<span>
									<Truncate text={file().name} reveal />, {sizeOf(file().size)}
								</span>
								<Button
									variant="ghost"
									size="sm"
									iconOnly
									aria-label={`Remove ${file().name}`}
									onClick={() => setImage(undefined)}
								>
									<Icon name="close" />
								</Button>
							</div>
						)}
					</Show>
					<fieldset class="fset field">
						<legend class="flab">Attached</legend>
						<div class="fb-ctx">
							<For each={items}>
								{(item) => (
									<label classList={{ off: off().has(item.id) }}>
										<input
											type="checkbox"
											checked={!off().has(item.id)}
											onChange={() => toggle(item.id)}
										/>
										<span class="fb-k">{item.label}</span>
										<span class="fb-v">{item.value}</span>
									</label>
								)}
							</For>
						</div>
					</fieldset>
				</div>
				<div class="ui-dialog-foot">
					<Button onClick={props.onClose}>Cancel</Button>
					<Button
						variant="primary"
						disabled={!message().trim() || phase() === "sending"}
						onClick={submit}
					>
						{phase() === "failed" ? "Try again" : "Send"}
					</Button>
				</div>
			</Show>
		</Dialog>
	);
};

export default FeedbackDialog;

const Sent = (props: { onClose: () => void }) => {
	const sent = createUniqueId();
	let done: HTMLButtonElement | undefined;
	onMount(() => {
		done?.focus();
		const timer = setTimeout(props.onClose, SENT_FOR);
		onCleanup(() => clearTimeout(timer));
	});
	return (
		<>
			<div class="ui-dialog-body fb-sent" role="status">
				<span class="fb-okmark">
					<Icon name="check" />
				</span>
				<h3 id={sent}>Sent</h3>
			</div>
			<div class="ui-dialog-foot">
				<Button
					ref={done}
					variant="primary"
					aria-describedby={sent}
					onClick={props.onClose}
				>
					Done
				</Button>
			</div>
		</>
	);
};
