import { inject } from "vitest";

declare module "vitest" {
	interface ProvidedContext {
		speedFactor: number;
	}
}

/** A frame budget measured on a development machine, scaled for the machine running the tests. */
export const budget = (ms: number) => ms * inject("speedFactor");
