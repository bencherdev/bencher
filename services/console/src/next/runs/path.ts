import { projectPath } from "../paths";

/** A run's page, under its project's Runs. */
export const runPath = (slug: string, version: string) =>
	`${projectPath(slug, "runs")}/${version}`;
