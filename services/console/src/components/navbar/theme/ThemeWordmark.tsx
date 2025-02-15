import { createMemo } from "solid-js";
import { BENCHER_WORDMARK } from "../../../util/ext";
import Wordmark from "../Wordmark";
import { themeWordmark } from "./theme";
import { themeSignal } from "./util";

const ThemeWordmark = (props: { id?: string }) => {
	const wordmark = createMemo(
		() => themeWordmark(themeSignal()),
		BENCHER_WORDMARK,
	);

	return <Wordmark id={props.id} src={wordmark()} />;
};

export default ThemeWordmark;
