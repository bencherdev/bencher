import * as core from "@actions/core";
import * as toolCache from "@actions/tool-cache";
import { chmod } from "fs/promises";

import swagger from "../../console/src/content/api/swagger.json";

const run = async () => {
	const cli_version = core.getInput("version");
	const { url, version } = into_url(cli_version);
	const toolPath = toolCache.find("bencher", version);
	if (toolPath) {
		core.addPath(toolPath);
	} else {
		core.info(`Downloading Bencher CLI ${version} from: ${url}`);
		await install(url, version);
	}
	core.info(`Bencher CLI ${version} installed!`);
};

const into_url = (cli_version: string) => {
	const version =
		cli_version === "latest"
			? swagger?.info?.version
			: cli_version.replace(/^v/, "");
	const url = `https://github.com/bencherdev/bencher/releases/download/v${version}/bencher`;
	return { url, version };
};

const install = async (url: string, version: string) => {
	const bencher = await toolCache.downloadTool(url);
	await chmod(bencher, 0o755);
	const cache = await toolCache.cacheFile(
		bencher,
		"bencher",
		"bencher",
		version,
	);
	core.addPath(cache);
};

run().catch((error) => {
	if (error instanceof Error) {
		core.setFailed(error.message);
	} else {
		core.setFailed(`${error}`);
	}
});
