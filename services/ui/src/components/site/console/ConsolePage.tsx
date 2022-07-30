import ConsoleMenu from "./menu/ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";

const ConsolePage = (props) => {
  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <ConsoleMenu
              project_slug={props.project_slug}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={props.handleProjectSlug}
            />
          </div>
          <div class="column">
            <ConsolePanel
              operation={props.operation}
              current_location={props.current_location}
              handleTitle={props.handleTitle}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={props.handleProjectSlug}
            />
          </div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
