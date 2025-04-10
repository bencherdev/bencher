import { createMemo } from "solid-js";
import { Theme } from "../../navbar/theme/theme";
import { theme } from "../../navbar/theme/util";

export interface Props {
	light: string;
	dark: string;
	height?: undefined | string;
	width?: undefined | string;
	alt: string;
}

const DocsImg = (props: Props) => {
	const src = createMemo(() => {
		switch (theme()) {
			case Theme.Light:
				return props.light;
			case Theme.Dark:
				return props.dark;
		}
	});

	return (
		<img
			src={src()}
			height={props.height}
			width={props.width}
			alt={props.alt}
		/>
	);
};

export default DocsImg;
