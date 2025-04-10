import { createMemo } from "solid-js";
import { BENCHER_WORDMARK } from "../../../util/ext";
import Wordmark from "../Wordmark";
import { themeWordmark } from "./theme";
import { theme } from "./util";

const ThemeWordmark = (props: { id?: string }) => {
	const wordmark = createMemo(() => themeWordmark(theme()), BENCHER_WORDMARK);

	return <Wordmark id={props.id} src={wordmark()} />;
};

export default ThemeWordmark;
