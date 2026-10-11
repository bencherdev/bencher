import type { QueryClient } from "@tanstack/solid-query";
import type { JsonConsoleProject } from "../../types/bencher";
import type { Api } from "../api";
import { consoleProjectQuery } from "../queries";

export const PLOTS_LIMIT = 64;

/** Keep the shell's count of pins in step with a pin made or removed here. */
export const countPins = (
	client: QueryClient,
	api: Api,
	slug: string,
	change: number,
) =>
	client.setQueryData<JsonConsoleProject>(
		consoleProjectQuery(api, slug).queryKey,
		(project) =>
			project && { ...project, plots: Math.max(0, project.plots + change) },
	);
