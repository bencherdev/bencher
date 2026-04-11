import { Language } from "../../i18n/ui";
import type Collection from "../../util/collection";

// This document was automatically translated by AI. It might not be accurate and might contain errors. If you find any errors, please <>open an issue on GitHub</>.
const Disclosure = (props: {
	collection?: Collection;
	slug: undefined | string;
	lang: undefined | string | Language;
}) => {
	const lang = (props.lang as Language) ?? Language.en;
	const page = `${props.collection ? `${props.collection}+` : ""}${props.slug
		?.split("/")
		?.join("+")}`;
	switch (lang) {
		case Language.en:
			return <></>;
		case Language.de:
			return <DisclosureDe page={page} />;
		case Language.es:
			return <DisclosureEs page={page} />;
		case Language.fr:
			return <DisclosureFr page={page} />;
		case Language.ja:
			return <DisclosureJa page={page} />;
		case Language.ko:
			return <DisclosureKo page={page} />;
		case Language.pt:
			return <DisclosurePt page={page} />;
		case Language.ru:
			return <DisclosureRu page={page} />;
		case Language.zh:
			return <DisclosureZh page={page} />;
	}
};

const DisclosureInner = (props: {
	bodyText: string;
	linkText: string;
	page: string;
	lang: Language;
}) => {
	return (
		<div class="box" style="margin-top: 4rem;">
			🤖 {props.bodyText}
			<a
				href={`https://github.com/bencherdev/bencher/issues/new?title=Issue+with+translation+to+${props.lang}&body=Issue+with+translation+to+${props.lang}+for+${props.page}&labels=documentation`}
				target="_blank"
				rel="noreferrer"
			>
				{props.linkText}
			</a>
			.
		</div>
	);
};

const DisclosureDe = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="Dieses Dokument wurde automatisch von KI übersetzt. Es ist möglicherweise nicht korrekt und kann Fehler enthalten. Wenn Sie Fehler finden, "
			linkText="öffnen Sie bitte ein Problem auf GitHub"
			page={props.page}
			lang={Language.de}
		/>
	);
};

const DisclosureEs = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="Este documento fue traducido automáticamente por IA. Puede que no sea exacto y contenga errores. Si encuentra algún error, "
			linkText="abra un problema en GitHub"
			page={props.page}
			lang={Language.es}
		/>
	);
};

const DisclosureFr = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="Ce document a été automatiquement traduit par IA. Il peut ne pas être précis et peut contenir des erreurs. Si vous trouvez des erreurs, veuillez "
			linkText="ouvrir une issue sur GitHub"
			page={props.page}
			lang={Language.fr}
		/>
	);
};

const DisclosureJa = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="このドキュメントは AI によって自動的に翻訳されました。 正確ではない可能性があり、間違いが含まれている可能性があります。 エラーを見つけた場合は、"
			linkText="GitHub で問題を開いてください。"
			page={props.page}
			lang={Language.ja}
		/>
	);
};

const DisclosureKo = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="이 문서는 AI에 의해 자동으로 번역되었습니다. 정확하지 않을 수도 있고 오류가 있을 수도 있습니다. 오류를 발견하면 "
			linkText="GitHub에서 문제를 열어주세요"
			page={props.page}
			lang={Language.ko}
		/>
	);
};

const DisclosurePt = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="Este documento foi traduzido automaticamente por IA. Pode não ser preciso e pode conter erros. Se você encontrar algum erro, "
			linkText="abra um problema no GitHub"
			page={props.page}
			lang={Language.pt}
		/>
	);
};

const DisclosureRu = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="Этот документ был автоматически переведён с помощью ИИ. Он может быть неточным и содержать ошибки. Если вы обнаружите какие-либо ошибки, "
			linkText="откройте проблему на GitHub"
			page={props.page}
			lang={Language.ru}
		/>
	);
};

const DisclosureZh = (props: { page: string }) => {
	return (
		<DisclosureInner
			bodyText="该文档由 AI 自动翻译。 它可能不准确并且可能包含错误。 如果您发现任何错误，请"
			linkText="在 GitHub 上提出问题"
			page={props.page}
			lang={Language.zh}
		/>
	);
};

export default Disclosure;
