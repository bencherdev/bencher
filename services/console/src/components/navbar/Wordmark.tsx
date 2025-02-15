import { BENCHER_WORDMARK } from "../../util/ext";

const Wordmark = (props: { id?: undefined | string; src?: string }) => (
	<img
		id={props.id}
		src={props.src ?? BENCHER_WORDMARK}
		width={150}
		height={28.25}
		aria-label="🐰 Bencher"
	/>
);

export default Wordmark;
