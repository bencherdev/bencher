import type { APIRoute } from "astro";

export function getStaticPaths() {
	return [
		{ params: { file: "install-cli.sh" } },
		{ params: { file: "install-cli.ps1" } },
	];
}

export const GET: APIRoute = ({ params, request: _ }) => {
	const content = "echo todo";
	const extension = params?.file?.split(".").pop();
	return new Response(content, {
		headers: {
			"Content-Type": `text/${extension}`,
		},
	});
};
