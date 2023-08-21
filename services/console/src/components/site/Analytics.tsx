export interface Props {
	id: undefined | string;
}
const Analytics = (props: Props) => {
	const scriptTag = () => {
		window.dataLayer = window.dataLayer || [];
		function gtag() {
			dataLayer.push(arguments);
		}
		gtag("js", new Date());
		gtag("config", props.id);
	};

	return (
		<>
			{" "}
			<script
				type="text/partytown"
				src={`https://www.googletagmanager.com/gtag/js?id=${props.id}`}
			></script>
			<script type="text/partytown">{scriptTag()}</script>
		</>
	);
};

export default Analytics;
