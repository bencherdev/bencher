import { Show, createSignal } from "solid-js";

export const COPY_TIMEOUT = 2110;

interface Props {
	text: string;
}

const CopyButton = (props: Props) => {
	const [copied, setCopied] = createSignal(false);

	return (
		<button
			class="button is-fullwidth"
			title="Copy to clipboard"
			onClick={(e) => {
				e.preventDefault();
				navigator.clipboard.writeText(props.text);
				setCopied(true);
				setTimeout(() => {
					setCopied(false);
				}, COPY_TIMEOUT);
			}}
		>
			<span class="icon-text">
				<Show
					when={copied()}
					fallback={
						<span class="icon">
							<i class="far fa-copy" />
						</span>
					}
				>
					<span class="icon has-text-success">
						<i class="far fa-check-circle" />
					</span>
				</Show>
				<span>Copy to Clipboard</span>
			</span>
		</button>
	);
};

export default CopyButton;
