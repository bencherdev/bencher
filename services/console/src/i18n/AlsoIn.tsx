import { Language, allLanguages, alsoIn, languageName } from "./ui";
import { langPath } from "./utils";

interface Props {
	lang: Language;
	path?: string;
}

export const AlsoInEn = (props: { path: string }) => {
	return <AlsoIn lang={Language.en} path={props.path} />;
};

const AlsoIn = (props: Props) => {
	return (
		<>
			<h3 class="title is-3">{alsoIn(props.lang)}</h3>
			<ul>
				{allLanguages.map(
					(language) =>
						props.lang !== language && (
							<li>
								<a href={`/docs/${langPath(language)}${props.path ?? ""}`}>
									{languageName(language)}
								</a>
							</li>
						),
				)}
			</ul>
		</>
	);
};

export default AlsoIn;
