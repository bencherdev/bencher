import Poster from "./Poster";
import PosterHeader from "./PosterHeader";

const PosterPanel = (props) => {
  return (
    <>
      <PosterHeader config={props.config?.header} pathname={props.pathname} />
      <Poster
        config={props.config?.form}
        pathname={props.pathname}
        path_params={props.path_params}
      />
    </>
  );
};

export default PosterPanel;
