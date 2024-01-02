import type { APIRoute } from "astro";

export const GET: APIRoute = async function GET() {
	return new Response("echo hello;", {
		headers: {
			"Content-Type": "text/ps1",
		},
	});
};
