import * as core from "@actions/core";
import * as toolCache from "@actions/tool-cache";
import { chmod } from "node:fs/promises";

import spec from "../../api/openapi.json";

const run = async () => {
	const { bin, url, semVer } = getUrl();
	const toolPath = toolCache.find("bencher", semVer);
	if (toolPath) {
		core.addPath(toolPath);
	} else {
		core.info(`Downloading Bencher CLI v${semVer} from: ${url}`);
		await install(bin, url, semVer);
	}
	core.info(`Bencher CLI v${semVer} installed!`);
};

const getUrl = () => {
	const semVer = getSemVer();
	const bin = getBin(semVer);
	const url = `https://github.com/bencherdev/bencher/releases/download/v${semVer}/${bin}`;
	return { bin, url, semVer };
};

const getSemVer = () => {
	// Gets the value of the input for `version`
	const version = core.getInput("version");
	switch (version) {
		//  `getInput` returns an empty string if the value is not defined.
		case "":
		// Except that it magically defaults to `latest` for `version`
		case "latest":
			// Get latest semantic version from openapi.json
			return `${spec?.info?.version}`;
		default:
			// Use user-specified version
			// Remove leading `v` if present
			// https://semver.org/#is-v123-a-semantic-version
			return version.replace(/^v/, "");
	}
};

const getBin = (semVer: string) => {
	const arch = (() => {
		switch (process.arch) {
			case "x64":
				return "x86-64";
			case "arm64":
				return "arm-64";
			default:
				throw new Error("Unsupported architecture");
		}
	})();
	switch (process.platform) {
		case "linux":
			return `bencher-v${semVer}-linux-${arch}`;
		case "darwin":
			return `bencher-v${semVer}-macos-${arch}`;
		case "win32":
			return `bencher-v${semVer}-windows-${arch}.exe`;
		default:
			throw new Error("Unsupported operating system");
	}
};

const BENCHER_CLI = "bencher";
const install = async (bin: string, url: string, semVer: string) => {
	const bencher = await toolCache.downloadTool(url);
	await chmod(bencher, 0o755);
	const cache = await toolCache.cacheFile(
		bencher,
		BENCHER_CLI,
		BENCHER_CLI,
		semVer,
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
