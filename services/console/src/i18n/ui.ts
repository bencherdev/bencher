export const defaultLang = "en";
export const showDefaultLang = false;

export const languages = {
	en: "English",
	fr: "Français",
};

export enum Language {
	en = "en",
	fr = "fr",
}

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
		case Language.fr:
			return "Didacticiel";
		case Language.en:
		default:
			return "Tutorial";
	}
};
