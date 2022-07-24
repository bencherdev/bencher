import ConsoleMenu from "./ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";

const ConsolePage = (props) => {
  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <ConsoleMenu />
          </div>
          <div class="column">
            <ConsolePanel
              current_location={props.current_location}
              handleTitle={props.handleTitle}
              handleRedirect={props.handleRedirect}
            />
          </div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
