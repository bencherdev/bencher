import { Button } from "../../console";
import Poster from "./Poster";
import PosterHeader from "./PosterHeader";

const PosterPanel = (props) => {
  return (
    <>
      <PosterHeader
        config={props.config?.header}
        title={props.config?.title}
        pathname={props.pathname}
        handleRedirect={props.handleRedirect}
      />
      <Poster config={props.config} handleRedirect={props.handleRedirect} />
    </>
  );
};

export default PosterPanel;
