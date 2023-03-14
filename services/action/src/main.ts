import * as core from "@actions/core";
import * as toolCache from "@actions/tool-cache";

const BENCHER_VERSION = "0.2.41";

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
		cli_version === "latest" ? BENCHER_VERSION : cli_version.replace(/^v/, "");
	const url = `https://github.com/bencherdev/bencher/releases/download/v${version}/bencher_${version}_amd64.deb`;
	return { url, version };
};

const install = async (url: string, version: string) => {
	const tar = await toolCache.downloadTool(url);
	const extracted = await toolCache.extractTar(tar);
	const cache = await toolCache.cacheDir(extracted, "bencher", version);
	core.addPath(cache);
};

run().catch((error) => {
	if (error instanceof Error) {
		core.setFailed(error.message);
	} else {
		core.setFailed(`${error}`);
	}
});
