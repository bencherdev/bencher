export const showDefaultLang = false;

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
		case Language.zh:
			return "教程";
		case Language.ru:
			return "учебника";
	}
};

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
		case Language.pt:
			return "Como";
		case Language.ko:
			return "어떻게";
		case Language.zh:
			return "如何";
		case Language.ru:
			return "Как";
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
		case Language.pt:
			return "Explicação";
		case Language.ko:
			return "설명";
		case Language.zh:
			return "解释";
		case Language.ru:
			return "Объяснение";
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
		case Language.pt:
			return "Referência";
		case Language.ko:
			return "참조";
		case Language.zh:
			return "参考";
		case Language.ru:
			return "Ссылка";
	}
};
