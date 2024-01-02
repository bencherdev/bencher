import { createEffect, createSignal } from "solid-js";
import { SwaggerUIBundle } from "swagger-ui-dist";
import { Language } from "../../i18n/ui";
import {
	BENCHER_CLOUD,
	BENCHER_SELF_HOSTED,
	isBencherCloud,
	swaggerSpec,
} from "../../util/ext";

export interface Props {
	apiUrl: string;
	lang: undefined | string | Language;
}

const apiEndpoint = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "API Endpoint";
		case Language.de:
			return "API Endpunkt";
		case Language.es:
			return "Punto final de la API";
		case Language.fr:
			return "Point de terminaison de l'API";
		case Language.ja:
			return "APIエンドポイント";
		case Language.ko:
			return "API 엔드 포인트";
		case Language.pt:
			return "Ponto de extremidade da API";
		case Language.ru:
			return "Конечная точка API";
		case Language.zh:
			return "API端点";
	}
};

const viewApiSpec = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "View OpenAPI Spec";
		case Language.de:
			return "OpenAPI-Spezifikation anzeigen";
		case Language.es:
			return "Ver especificación OpenAPI";
		case Language.fr:
			return "Voir la spécification OpenAPI";
		case Language.ja:
			return "OpenAPI仕様を表示";
		case Language.ko:
			return "OpenAPI 사양보기";
		case Language.pt:
			return "Ver especificação OpenAPI";
		case Language.ru:
			return "Просмотреть спецификацию OpenAPI";
		case Language.zh:
			return "查看OpenAPI规范";
	}
};

const rustClient = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Rust Client";
		case Language.de:
			return "Rust Client";
		case Language.es:
			return "Cliente Rust";
		case Language.fr:
			return "Client Rust";
		case Language.ja:
			return "Rustクライアント";
		case Language.ko:
			return "Rust 클라이언트";
		case Language.pt:
			return "Cliente Rust";
		case Language.ru:
			return "Клиент Rust";
		case Language.zh:
			return "Rust客户端";
	}
};

const rustClientText = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "If you're writing in Rust consider using the Bencher Rust API Client.";
		case Language.de:
			return "Wenn Sie in Rust schreiben, sollten Sie den Bencher Rust API Client verwenden.";
		case Language.es:
			return "Si está escribiendo en Rust, considere usar el cliente de API Rust de Bencher.";
		case Language.fr:
			return "Si vous écrivez en Rust, envisagez d'utiliser le client API Rust de Bencher.";
		case Language.ja:
			return "Rustで書いている場合は、Bencher Rust APIクライアントを使用することを検討してください。";
		case Language.ko:
			return "Rust로 작성하는 경우 Bencher Rust API 클라이언트를 사용하는 것이 좋습니다.";
		case Language.pt:
			return "Se você está escrevendo em Rust, considere usar o cliente de API Rust do Bencher.";
		case Language.ru:
			return "Если вы пишете на Rust, рекомендуем использовать клиент API Rust Bencher.";
		case Language.zh:
			return "如果您使用Rust编写，请考虑使用Bencher Rust API Client。";
	}
};

const SwaggerPanel = (props: Props) => {
	const lang = (props.lang as Language) ?? Language.en;
	const [url, setUrl] = createSignal("");

	createEffect(() => {
		const [url, swagger] = swaggerSpec(props.apiUrl);
		setUrl(url);
		SwaggerUIBundle({
			dom_id: "#swagger",
			spec: swagger,
			layout: "BaseLayout",
		});
	});

	return (
		<div class="content">
			<blockquote>
				<nav class="level">
					<div class="level-left">
						<div class="level-item">
							<p>
								🐰 {isBencherCloud() ? BENCHER_CLOUD : BENCHER_SELF_HOSTED}{" "}
								{apiEndpoint(lang)}:{" "}
								<code>
									<a
										href={`${url()}/v0/server/version`}
										target="_blank"
										rel="noreferrer"
									>
										{url()}
									</a>
								</code>
							</p>
						</div>
					</div>

					<div class="level-right">
						<a
							class="button is-fullwidth"
							href={`${url()}/v0/server/spec`}
							target="_blank"
							rel="noreferrer"
						>
							{viewApiSpec(lang)}
						</a>
					</div>
				</nav>
			</blockquote>
			<h2>🦀 {rustClient(lang)}</h2>
			<p>{rustClientText(lang)}</p>
			<code>
				bencher_client = {"{"} git = "https://github.com/bencherdev/bencher",
				branch = "main" {"}"}
			</code>
			<hr />
			<div id="swagger" />
			<br />
		</div>
	);
};

export default SwaggerPanel;
