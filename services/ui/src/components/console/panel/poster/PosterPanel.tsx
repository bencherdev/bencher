import { Button } from "../../console";
import Poster from "./Poster";
import PosterHeader from "./PosterHeader";

const PosterPanel = (props) => {
  return (
    <>
      <PosterHeader
        title={props.config?.title}
        back_path={props.config?.buttons?.[Button.BACK]?.path}
        handleRedirect={props.handleRedirect}
      />
      <Poster config={props.config} handleRedirect={props.handleRedirect} />
    </>
  );
};

export default PosterPanel;
