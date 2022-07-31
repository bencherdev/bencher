import { Button } from "../../console";
import PosterHeader from "./PosterHeader";

const PosterPanel = (props) => {
  return (
    <>
      <PosterHeader
        title={props.config?.title}
        back_path={props.config?.buttons?.[Button.BACK]?.path}
        handleRedirect={props.handleRedirect}
      />
      {/* <Deck data={deck_data()} /> */}
    </>
  );
};

export default PosterPanel;
