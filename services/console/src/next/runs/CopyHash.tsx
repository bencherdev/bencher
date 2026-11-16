import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import { createSignal, onCleanup } from "solid-js";

/** Copies a version's whole hash, which the page shows only short, from right after it; it shows a check, and says when it has copied. */
const CopyHash = (props: { hash: string }) => {
	const [copied, setCopied] = createSignal(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	const copy = () =>
		navigator.clipboard
			.writeText(props.hash)
			.then(() => {
				clearTimeout(timer);
				setCopied(true);
				timer = setTimeout(() => setCopied(false), 2_000);
			})
			.catch(() => {});
	return (
		<span class="rn-copy" data-fixed>
			<Button
				size="sm"
				variant="ghost"
				iconOnly
				aria-label={`${copied() ? "Copied" : "Copy"} hash`}
				onClick={copy}
			>
				<Icon name={copied() ? "check" : "copy"} />
			</Button>
			<span class="sr-only" role="status">
				{copied() ? "Copied the hash" : ""}
			</span>
		</span>
	);
};

export default CopyHash;
