import { beforeEach } from "vitest";
import { commands } from "vitest/browser";

declare module "vitest/browser" {
	interface BrowserCommands {
		restPointer: () => Promise<void>;
	}
}

// The mouse stays where the last test left it, and a row drawn under it would see an intent no test made.
beforeEach(() => commands.restPointer());
