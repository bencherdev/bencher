export enum Author {
	everett = "Everett Pompeii",
}

export const twitter = (author: Author) => {
	switch (author) {
		case Author.everett:
			return "@epompeii";
	}
};

export const TWITTER_BENCHER = "@bencherdev";
