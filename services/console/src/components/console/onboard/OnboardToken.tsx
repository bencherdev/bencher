import bencher_valid_init, { type InitOutput } from "bencher_valid";

import { createEffect, createMemo, createResource } from "solid-js";
import { authUser } from "../../../util/auth";
import { useNavigate, useSearchParams } from "../../../util/url";
import { validJwt } from "../../../util/valid";
import { httpGet, httpPost } from "../../../util/http";
import type { JsonNewToken, JsonToken } from "../../../types/bencher";

export interface Props {
	apiUrl: string;
}

const OnboardToken = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const [searchParams, _setSearchParams] = useSearchParams();
	const navigate = useNavigate();

	const tokensFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
		};
	});
	const getTokens = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
	}) => {
		if (!fetcher.bencher_valid) {
			return undefined;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		const path = `/v0/users/${user?.user?.uuid}/tokens`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [apiTokens] = createResource<null | JsonToken[]>(
		tokensFetcher,
		getTokens,
	);

	const tokenFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			tokens: apiTokens(),
		};
	});
	const getToken = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
		tokens: undefined | null | JsonToken[];
	}) => {
		if (!fetcher.bencher_valid) {
			return undefined;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		if (fetcher.tokens === undefined) {
			return undefined;
		}
		if (fetcher.tokens === null) {
			return null;
		}
		const now = new Date();
		for (let i = 0; i < fetcher.tokens.length; i++) {
			const expiration = fetcher.tokens[i]?.expiration;
			if (!expiration) {
				continue;
			}
			const expirationDate = new Date(expiration);
			if (expirationDate > now) {
				return fetcher.tokens[i];
			}
		}
		const path = `/v0/users/${user?.user?.uuid}/tokens`;
		const data: JsonNewToken = {
			name: `${user?.user?.name}'s token`,
		};
		return await httpPost(props.apiUrl, path, fetcher.token, data)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [apiToken] = createResource<null | JsonToken[]>(tokenFetcher, getToken);

	return (
		<>
			<figure class="frame">
				<pre data-language="plaintext">
					<code>
						<div class="code">{apiToken()?.token}</div>
					</code>
				</pre>
				<button
					class="button is-outlined is-fullwidth"
					title="Copy API token to clipboard"
					onClick={(e) => {
						e.preventDefault;
						navigator.clipboard.writeText(apiToken()?.token);
					}}
				>
					<span class="icon-text">
						<span class="icon">
							<i class="far fa-copy"></i>
						</span>
						<span>Copy to Clipboard</span>
					</span>
				</button>
			</figure>
			<br />
			<br />
			<a class="button is-primary is-fullwidth" href="/console/onboard/project">
				<span class="icon-text">
					<span>Next Step</span>
					<span class="icon">
						<i class="fas fa-chevron-right"></i>
					</span>
				</span>
			</a>
		</>
	);
};

export default OnboardToken;
