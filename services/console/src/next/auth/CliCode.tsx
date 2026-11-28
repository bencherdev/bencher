import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import { Show, createSignal, onMount } from "solid-js";

/** An authorization code, as the API makes one: its prefix and 30 ASCII letters or digits. */
const CODE = /^bencher_code_[A-Za-z0-9]{30}$/;

/** The code, if it is one the API could have made. */
export const pasteable = (code: string | undefined) =>
	code && CODE.test(code) ? code : undefined;

/**
 * The authorization code for the Bencher CLI to paste, which is useless
 * without the verifier the CLI holds. The page sends it nowhere: Copy puts it
 * on the clipboard alone. Anything but a code is `malformed`, which shows
 * nothing.
 */
const CliCode = (props: {
	code: string | undefined;
	malformed: () => void;
	copy?: ((code: string) => Promise<void>) | undefined;
}) => {
	const [copied, setCopied] = createSignal(false);
	const code = pasteable(props.code);
	onMount(() => {
		if (!code) {
			props.malformed();
		}
	});
	const copy = (text: string) =>
		(props.copy ?? ((value) => navigator.clipboard.writeText(value)))(
			text,
		).then(
			() => setCopied(true),
			() => setCopied(false),
		);
	return (
		<Show when={code}>
			{(code) => (
				<>
					<Heading level={1} size="xl">
						CLI code
					</Heading>
					<div class="secret">
						<span>{code()}</span>
						<Button
							size="sm"
							aria-label={copied() ? "Code copied" : "Copy the code"}
							onClick={() => void copy(code())}
						>
							{copied() ? "Copied" : "Copy"}
						</Button>
					</div>
				</>
			)}
		</Show>
	);
};

export default CliCode;
