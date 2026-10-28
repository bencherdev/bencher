import { batch } from "solid-js";

/** Reads an element's layout and returns what to write because of it, if anything. */
type Read = () => (() => void) | undefined;

const reads = new Map<Element, Read>();
let observer: ResizeObserver | undefined;
let delivering = false;

// Every read comes before any write, so one pass costs one layout.
const run = (elements: Iterable<Element>) => {
	const writes = [...elements].map((element) => reads.get(element)?.());
	delivering = true;
	batch(() => {
		for (const write of writes) {
			write?.();
		}
	});
	delivering = false;
};

const deliver = (entries: ResizeObserverEntry[]) =>
	run(entries.map((entry) => entry.target));

/**
 * One ResizeObserver for the whole design system. A write must not resize an
 * observed element in the same frame; one that might waits for the next.
 * Returns the cleanup.
 */
export const watchSize = (element: Element, read: Read) => {
	if (observer === undefined) {
		observer = new ResizeObserver(deliver);
		// A web font that lands can change a text's width without resizing its box.
		document.fonts?.addEventListener("loadingdone", () => run(reads.keys()));
	}
	reads.set(element, read);
	const start = () => {
		if (reads.get(element) === read) {
			observer?.observe(element);
		}
	};
	// Observed from inside a delivery, its first notice would be skipped.
	if (delivering) {
		requestAnimationFrame(start);
	} else {
		start();
	}
	return () => {
		if (reads.get(element) === read) {
			reads.delete(element);
			observer?.unobserve(element);
		}
	};
};
