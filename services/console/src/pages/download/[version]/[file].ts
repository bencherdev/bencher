export const prerender = false;

export async function GET({ params, redirect }) {
	// Make the leading `v` optional
	// Remove leading `v` if present
	// https://semver.org/#is-v123-a-semantic-version
	const semVer = params?.version?.replace(/^v/, "");
	const file = params?.file;
	const url = `https://github.com/bencherdev/bencher/releases/download/v${semVer}/${file}`;
	return redirect(url, 307);
}
