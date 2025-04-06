import { adapter } from "./util";
import { adapterCommand } from "./adapter";

const BencherRun = () => {
    return (
        <code>{adapterCommand(adapter())}</code>
    );
}

export default BencherRun;