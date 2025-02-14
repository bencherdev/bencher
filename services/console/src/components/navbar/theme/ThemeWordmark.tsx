import { createMemo } from "solid-js";
import { BENCHER_WORDMARK, BENCHER_WORDMARK_ID } from "../../../util/ext";
import { themeWordmark } from "./theme";
import { themeSignal } from "./util";

const ThemeWordmark = () => {
	const wordmark = createMemo(
		() => themeWordmark(themeSignal()),
		BENCHER_WORDMARK,
	);

	return (
		<img
			id={BENCHER_WORDMARK_ID}
			src={wordmark()}
			width="150"
			height="28.25"
			alt="🐰 Bencher"
		/>
	);
};

export default ThemeWordmark;
