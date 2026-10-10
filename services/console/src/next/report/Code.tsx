import Button from "@bencherdev/ui/Button";
import { For, Show, createSignal, onCleanup } from "solid-js";

/** A command to copy, its rows drawn with the ones that matter flagged. */
const Code = (props: {
	/** What Copy copies, for its accessible name. */
	name: string;
	text: string;
	code: readonly { text: string; flag: boolean }[];
}) => {
	const [copy, setCopy] = createSignal<"copied" | "refused">();
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	const write = async () => {
		clearTimeout(timer);
		try {
			await navigator.clipboard.writeText(props.text);
		} catch {
			setCopy("refused");
			return;
		}
		setCopy("copied");
		timer = setTimeout(() => setCopy(), 2_000);
	};
	return (
		<>
			<div class="code">
				<For each={props.code}>
					{(row) => (
						<span class="rp-snip" classList={{ "rp-flag": row.flag }}>
							{row.text}
						</span>
					)}
				</For>
				<Button size="sm" aria-label={`Copy ${props.name}`} onClick={write}>
					{copy() === "copied" ? "Copied" : "Copy"}
				</Button>
			</div>
			<Show when={copy() === "refused"}>
				<p class="sm rp-refused" role="alert">
					The browser refused to copy; select the command to copy it.
				</p>
			</Show>
		</>
	);
};

export default Code;
