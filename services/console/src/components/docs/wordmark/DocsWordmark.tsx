import { createMemo } from "solid-js";
import { BENCHER_WORDMARK, BENCHER_WORDMARK_ID } from "../../../util/ext";
import { themeWordmark } from "../../navbar/theme/theme";
import { theme } from "../../navbar/theme/util";

const DocsWordmark = () => {
	const wordmark = createMemo(() => themeWordmark(theme()), BENCHER_WORDMARK);

	return (
		<img
			id={BENCHER_WORDMARK_ID}
			src={wordmark()}
			width="90%"
			aria-label="🐰 Bencher"
		/>
	);
};

export default DocsWordmark;
