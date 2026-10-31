import Button from "@bencherdev/ui/Button";
import { createSignal, onCleanup } from "solid-js";

/** Copies a version's whole hash, which the page shows only short; it keeps the width of either word, and says when it has copied. */
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
				aria-label={`${copied() ? "Copied" : "Copy"} hash`}
				onClick={copy}
			>
				<span class="rn-copyword" data-copied={copied() ? "" : undefined}>
					<span class="rn-copyto">Copy</span>
					<span class="rn-copied">Copied</span>
				</span>
			</Button>
			<span class="sr-only" role="status">
				{copied() ? "Copied the hash" : ""}
			</span>
		</span>
	);
};

export default CopyHash;
