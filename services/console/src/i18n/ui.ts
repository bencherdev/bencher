export const showDefaultLang = false;

export const langButtonId = "lang-button";
export const langBoxId = "lang-box";

export enum Language {
	de = "de",
	en = "en",
	es = "es",
	fr = "fr",
	ja = "ja",
	ko = "ko",
	pt = "pt",
	ru = "ru",
	zh = "zh",
}
export const defaultLang = Language.en;

// As of right now these are the i18n languages for the GitHub Docs
// https://github.blog/2020-07-02-how-we-launched-docs-github-com/#internationalized-docs
export const otherLanguages = [
	Language.zh,
	Language.es,
	Language.pt,
	Language.ru,
	Language.ja,
	Language.fr,
	Language.de,
	Language.ko,
];

export const allLanguages = [Language.en, ...otherLanguages];

export const alsoIn = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Also available in:";
		case Language.de:
			return "Auch erhältlich in:";
		case Language.es:
			return "También disponible en:";
		case Language.fr:
			return "Également disponible en:";
		case Language.ja:
			return "以下でも利用可能です:";
		case Language.ko:
			return "다음에서도 사용 가능:";
		case Language.pt:
			return "Também disponível em:";
		case Language.ru:
			return "Также доступно в:";
		case Language.zh:
			return "也可用于:";
	}
};

export const languageName = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "English";
		case Language.de:
			return "Deutsch";
		case Language.es:
			return "Español";
		case Language.fr:
			return "Français";
		case Language.ja:
			return "日本語";
		case Language.ko:
			return "한국어";
		case Language.pt:
			return "Português do Brasil";
		case Language.ru:
			return "Русский";
		case Language.zh:
			return "简体中文";
	}
};

export const tutorial = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Tutorial";
		case Language.de:
			return "Lernprogramm";
		case Language.es:
			return "Tutorial";
		case Language.fr:
			return "Didacticiel";
		case Language.ja:
			return "チュートリアル";
		case Language.ko:
			return "자습서 작";
		case Language.pt:
			return "Tutorial";
		case Language.ru:
			return "учебника";
		case Language.zh:
			return "教程";
	}
};
export const signupId = "sign-up-bencher-cloud";

export const howTo = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "How To";
		case Language.de:
			return "Wie man";
		case Language.es:
			return "Cómo";
		case Language.fr:
			return "Comment";
		case Language.ja:
			return "方法";
		case Language.ko:
			return "어떻게";
		case Language.pt:
			return "Como";
		case Language.ru:
			return "Как";
		case Language.zh:
			return "如何";
	}
};

export const explanation = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Explanation";
		case Language.de:
			return "Erklärung";
		case Language.es:
			return "Explicación";
		case Language.fr:
			return "Explication";
		case Language.ja:
			return "説明";
		case Language.ko:
			return "설명";
		case Language.pt:
			return "Explicação";
		case Language.ru:
			return "Объяснение";
		case Language.zh:
			return "解释";
	}
};

export const reference = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Reference";
		case Language.de:
			return "Verweis";
		case Language.es:
			return "Referencia";
		case Language.fr:
			return "Référence";
		case Language.ja:
			return "関連項目";
		case Language.ko:
			return "참조";
		case Language.pt:
			return "Referência";
		case Language.ru:
			return "Ссылка";
		case Language.zh:
			return "参考";
	}
};

export const architecture = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Architecture";
		case Language.de:
			return "Architektur";
		case Language.es:
			return "Arquitectura";
		case Language.fr:
			return "Architecture";
		case Language.ja:
			return "アーキテクチャ";
		case Language.ko:
			return "아키텍처";
		case Language.pt:
			return "Arquitetura";
		case Language.ru:
			return "Архитектура";
		case Language.zh:
			return "架构";
	}
};

export const schema = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Schema";
		case Language.de:
			return "Schema";
		case Language.es:
			return "Esquema";
		case Language.fr:
			return "Schéma";
		case Language.ja:
			return "スキーマ";
		case Language.ko:
			return "스키마";
		case Language.pt:
			return "Esquema";
		case Language.ru:
			return "Схема";
		case Language.zh:
			return "模式";
	}
};

export const benchmarking = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Benchmarking";
		case Language.de:
			return "Benchmarking";
		case Language.es:
			return "Evaluación Comparativa";
		case Language.fr:
			return "Analyse Comparative";
		case Language.ja:
			return "ベンチマーク";
		case Language.ko:
			return "벤치마킹";
		case Language.pt:
			return "Avaliação Comparativa";
		case Language.ru:
			return "Бенчмаркинг";
		case Language.zh:
			return "标杆管理";
	}
};

export const trackInCi = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Track in CI";
		case Language.de:
			return "Nachverfolgen in CI";
		case Language.es:
			return "Seguimiento en CI";
		case Language.fr:
			return "Suivi dans l'intégration continue (CI)";
		case Language.ja:
			return "CIでのトラック";
		case Language.ko:
			return "CI에서 추적하기";
		case Language.pt:
			return "Rastrear no CI";
		case Language.ru:
			return "Отслеживание в CI";
		case Language.zh:
			return "在 CI 中跟踪";
	}
};

export const caseStudy = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Case Study";
		case Language.de:
			return "Fallstudie";
		case Language.es:
			return "Estudio de Caso";
		case Language.fr:
			return "Étude de cas";
		case Language.ja:
			return "事例研究";
		case Language.ko:
			return "사례 연구";
		case Language.pt:
			return "Estudo de Caso";
		case Language.ru:
			return "Кейс-стади";
		case Language.zh:
			return "案例研究";
	}
};

export const engineering = (lang: Language) => {
	switch (lang) {
		case Language.en:
			return "Engineering";
		case Language.de:
			return "Ingenieurwesen";
		case Language.es:
			return "Ingeniería";
		case Language.fr:
			return "Ingénierie";
		case Language.ja:
			return "エンジニアリング";
		case Language.ko:
			return "공학";
		case Language.pt:
			return "Engenharia";
		case Language.ru:
			return "Инженерия";
		case Language.zh:
			return "工程";
	}
};
