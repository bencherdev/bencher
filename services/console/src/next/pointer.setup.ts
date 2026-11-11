import { beforeEach } from "vitest";
import { commands } from "vitest/browser";

declare module "vitest/browser" {
	interface BrowserCommands {
		restPointer: () => Promise<void>;
		mouseDown: () => Promise<void>;
		mouseUp: () => Promise<void>;
		/**
		 * Real fingers, by difference from those already down: `touchStart`
		 * presses only new points, `touchMove` moves the points given and
		 * presses any not down, `touchEnd` lifts the points given or every
		 * finger when given none, and `touchCancel` cancels every finger.
		 */
		touch: (
			type: "touchStart" | "touchMove" | "touchEnd" | "touchCancel",
			points: { id: number; x: number; y: number }[],
		) => Promise<void>;
	}
}

// The mouse stays where the last test left it, and a row drawn under it would see an intent no test made.
beforeEach(() => commands.restPointer());
