import ConsoleMenu from "./ConsoleMenu";
import ConsoleTable from "./panel/ConsoleTable";

const ConsolePage = (props) => {
  props.handleTitle("Bencher Console - Track Your Benchmarks");

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <ConsoleMenu />
          </div>
          <div class="column">
            <ConsoleTable />
          </div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
