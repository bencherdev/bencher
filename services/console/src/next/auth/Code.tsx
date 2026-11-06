import { Index } from "solid-js";

export const CODE_LENGTH = 6;

export const emptyCode = () => Array.from({ length: CODE_LENGTH }, () => "");

/**
 * Six boxes, a digit each. A typed digit moves on to the next box, a pasted
 * code fills every box from the first, and the sixth digit sends the code.
 */
const Code = (props: {
	digits: readonly string[];
	disabled?: boolean | undefined;
	invalid?: boolean | undefined;
	onDigits: (digits: string[]) => void;
	onCode: (code: string) => void;
}) => {
	const boxes: HTMLInputElement[] = [];
	const fill = (from: number, digits: string) => {
		const next = [...props.digits];
		let at = from;
		for (const digit of digits) {
			if (at >= CODE_LENGTH) {
				break;
			}
			next[at] = digit;
			at += 1;
		}
		show(next);
		if (at === CODE_LENGTH && next.every((digit) => digit)) {
			props.onCode(next.join(""));
		} else {
			boxes[Math.min(at, CODE_LENGTH - 1)]?.focus();
		}
	};
	const show = (next: string[]) => {
		// A box keeps what the reader typed until it is set again, so each is set here.
		next.forEach((digit, index) => {
			const box = boxes[index];
			if (box) {
				box.value = digit;
			}
		});
		props.onDigits(next);
	};
	return (
		<fieldset class="au-code" aria-label="Code">
			<Index each={props.digits}>
				{(digit, index) => (
					<input
						ref={(box) => {
							boxes[index] = box;
						}}
						class="au-digit"
						type="text"
						inputmode="numeric"
						autocomplete={index === 0 ? "one-time-code" : "off"}
						aria-label={`Digit ${index + 1}`}
						aria-invalid={props.invalid ? "true" : undefined}
						disabled={props.disabled}
						value={digit()}
						onFocus={(event) => event.currentTarget.select()}
						onInput={(event) => {
							const box = event.currentTarget;
							const typed = box.value.replace(/\D/g, "");
							// A key typed into a filled box replaces its digit.
							const key =
								event.inputType === "insertText" &&
								/^\d$/.test(event.data ?? "")
									? (event.data ?? "")
									: typed;
							if (key) {
								fill(index, key);
							} else {
								const next = [...props.digits];
								next[index] = "";
								show(next);
							}
						}}
						onPaste={(event) => {
							event.preventDefault();
							const pasted =
								event.clipboardData?.getData("text").replace(/\D/g, "") ?? "";
							if (pasted) {
								fill(pasted.length >= CODE_LENGTH ? 0 : index, pasted);
							}
						}}
						onKeyDown={(event) => {
							if (
								event.key === "Backspace" &&
								!props.digits[index] &&
								index > 0
							) {
								event.preventDefault();
								const next = [...props.digits];
								next[index - 1] = "";
								show(next);
								boxes[index - 1]?.focus();
							} else if (event.key === "ArrowLeft" && index > 0) {
								event.preventDefault();
								boxes[index - 1]?.focus();
							} else if (
								event.key === "ArrowRight" &&
								index < CODE_LENGTH - 1
							) {
								event.preventDefault();
								boxes[index + 1]?.focus();
							}
						}}
					/>
				)}
			</Index>
		</fieldset>
	);
};

export default Code;
