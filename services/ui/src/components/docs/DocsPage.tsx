import DocsMenu from "./DocsMenu";
import DocsPanel from "./DocsPanel";

const DocsPage = (props) => {
  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          {/* <div class="column is-one-fifth">
            <DocsMenu page={props.page} />
          </div> */}
          {/* <div class="column">
            <DocsPanel page={props.page} />
          </div> */}
        </div>
      </div>
    </section>
  );
};

export default DocsPage;
