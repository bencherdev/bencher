import { createEffect, createSignal, on } from "solid-js";

/**
 * Says the page's title when the router moves to another page, as a full page
 * load would; the browser itself reads the first one.
 */
const Announcer = (props: { path: string; title: string }) => {
	const [said, say] = createSignal("");
	createEffect(
		on(
			() => props.path,
			() => say(props.title),
			{ defer: true },
		),
	);
	return (
		<p class="sr-only" role="status">
			{said()}
		</p>
	);
};

export default Announcer;
