import { adapter } from "./util";
import { adapterCommand } from "./adapter";

const BencherRun = (props: { isConsole: boolean }) => {
	return <code>{adapterCommand(props.isConsole, adapter())}</code>;
};

export default BencherRun;
