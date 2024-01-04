import type { APIRoute } from "astro";

export function getStaticPaths() {
	return [
		{ params: { file: "install-cli.sh" } },
		{ params: { file: "install-cli.ps1" } },
	];
}

export const GET: APIRoute = ({ params, redirect }) => {
	return redirect(`/${params.file}`, 307);
};
