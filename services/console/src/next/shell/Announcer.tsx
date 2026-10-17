import { createEffect, createSignal, on } from "solid-js";

// A later announcer comes with a move that swapped the layout.
let mounted = false;

/**
 * Says the page's title when the router moves to another page, as a full page
 * load would; the browser itself reads the first one.
 */
const Announcer = (props: { path: string; title: string }) => {
	const [said, say] = createSignal("");
	const first = !mounted;
	mounted = true;
	createEffect(
		on(
			() => props.path,
			() => say(props.title),
			{ defer: first },
		),
	);
	return (
		<p class="sr-only" role="status">
			{said()}
		</p>
	);
};

export default Announcer;
