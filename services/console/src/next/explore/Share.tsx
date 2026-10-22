import Button from "@bencherdev/ui/Button";
import Menu, { MenuItem } from "@bencherdev/ui/Menu";
import { Show, createSignal } from "solid-js";
import ShareIcon from "./ShareIcon";

/** What Share copied: the page's link, or the embed's frame. */
export type Copied = "link" | "embed";

const COPIED = { link: "Link copied", embed: "Embed code copied" } as const;

/** Share a plot: a public project's offers its link and its embed, a private one's copies the link. */
const Share = (props: {
	/** The project is public, so anyone can load its embed. */
	embeddable: boolean;
	copied: Copied | undefined;
	disabled?: boolean | undefined;
	onCopy: (what: Copied) => void;
}) => {
	const [open, setOpen] = createSignal(false);
	let button: HTMLButtonElement | undefined;
	const copy = (what: Copied) => {
		setOpen(false);
		props.onCopy(what);
	};
	return (
		<span class="share">
			<Button
				ref={button}
				size="sm"
				disabled={props.disabled}
				aria-haspopup={props.embeddable ? "menu" : undefined}
				aria-expanded={props.embeddable ? open() : undefined}
				onClick={() => (props.embeddable ? setOpen(!open()) : copy("link"))}
			>
				<ShareIcon />
				{props.copied ? COPIED[props.copied] : "Share"}
			</Button>
			<Show when={props.embeddable && open()}>
				<Menu
					label="Share"
					anchor={button}
					align="end"
					onClose={() => setOpen(false)}
				>
					<MenuItem onClick={() => copy("link")}>Copy link</MenuItem>
					<MenuItem onClick={() => copy("embed")}>Copy embed code</MenuItem>
				</Menu>
			</Show>
		</span>
	);
};

export default Share;
