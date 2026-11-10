import { Show } from "solid-js";

/** The wordmark's images, one per theme. */
export interface Wordmarks {
	dark: string;
	light: string;
}

/** The Bencher wordmark that ends the key's head. */
export interface PlotMark {
	/** In place of the site's own, for a page that may load images only from itself. */
	wordmarks?: Wordmarks | undefined;
	/** Opens bencher.dev, for a plot met outside Bencher. */
	link?: boolean | undefined;
}

const SITE: Wordmarks = {
	dark: "/wordmark-dark.svg",
	light: "/wordmark-light.svg",
};

/** Both themes' images, so a theme switch draws the other at once. */
const Images = (props: { wordmarks: Wordmarks | undefined }) => (
	<>
		<img src={(props.wordmarks ?? SITE).dark} alt="" />
		<img src={(props.wordmarks ?? SITE).light} alt="" />
	</>
);

const Mark = (props: PlotMark) => (
	<Show
		when={props.link}
		fallback={
			<span class="pl-mark">
				<Images wordmarks={props.wordmarks} />
			</span>
		}
	>
		<a
			class="pl-mark"
			href="https://bencher.dev"
			target="_blank"
			rel="noopener"
			aria-label="Bencher"
		>
			<Images wordmarks={props.wordmarks} />
		</a>
	</Show>
);

export default Mark;
