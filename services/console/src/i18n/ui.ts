export const defaultLang = "en";
export const showDefaultLang = false;

export const languages = {
	en: "English",
	es: "Español",
	fr: "Français",
};

export enum Language {
	en = "en",
	es = "es",
	fr = "fr",
	pt = "pt",
}

export const otherLanguages = [Language.es, Language.fr, Language.pt];

export const ui = {
	en: {
		"nav.home": "Home",
		"nav.about": "About",
		"nav.twitter": "Twitter",
	},
	fr: {
		"nav.home": "Accueil",
		"nav.about": "À propos",
	},
} as const;

export const tutorial = (lang: Language) => {
	switch (lang) {
		case Language.es:
			return "Tutorial";
		case Language.fr:
			return "Didacticiel";
		case Language.pt:
			return "Tutorial";
		case Language.en:
		default:
			return "Tutorial";
	}
};
