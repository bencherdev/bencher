export const showDefaultLang = false;

export enum Language {
	de = "de",
	en = "en",
	es = "es",
	fr = "fr",
	ja = "ja",
	pt = "pt",
	ru = "ru",
	zh = "zh",
}
export const defaultLang = Language.en;

export const otherLanguages = [
	Language.de,
	Language.es,
	Language.fr,
	Language.ja,
	Language.pt,
	Language.ru,
	Language.zh,
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
		case Language.pt:
			return "Português";
		case Language.ru:
			return "Русский";
		case Language.zh:
			return "中文";
	}
};

export const tutorial = (lang: Language) => {
	switch (lang) {
		case Language.de:
			return "Lernprogramm";
		case Language.es:
			return "Tutorial";
		case Language.fr:
			return "Didacticiel";
		case Language.ja:
			return "チュートリアル";
		case Language.pt:
			return "Tutorial";
		case Language.zh:
			return "教程";
		case Language.ru:
			return "Обучение";
		case Language.en:
		default:
			return "Tutorial";
	}
};
