import { createContext } from "solid-js";

/** Writes the query cache to the browser now: await it before a navigation that leaves the app. */
export const FlushContext = createContext<() => Promise<void>>(() =>
	Promise.resolve(),
);
