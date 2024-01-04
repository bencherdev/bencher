import type { APIRoute } from "astro";
import InstallCliPs1 from "../../../../cli/install-cli.ps1";
import InstallCliSh from "../../../../cli/install-cli.sh";

export function getStaticPaths() {
	return [
		{ params: { file: "install-cli.sh" } },
		{ params: { file: "install-cli.ps1" } },
	];
}

export const GET: APIRoute = ({ params, redirect }) => {
	const file = getFile(params?.file);
	return redirect(file, 307);
};

const getFile = (file: undefined | string) => {
	switch (file) {
		case "install-cli.sh":
			return InstallCliSh;
		case "install-cli.ps1":
			return InstallCliPs1;
		default:
			return "/404";
	}
}