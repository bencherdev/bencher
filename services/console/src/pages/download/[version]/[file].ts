export const prerender = false;

export async function GET({ params }) {
	const content = `echo "todo ${params?.file} ${params?.version}";`;
	return new Response(content, {
		headers: {
			"Content-Type": "application/binary",
		},
	});
}
