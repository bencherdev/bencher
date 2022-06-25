import { LinePlot } from "../../plot/LinePlot";

const LandingPage = (props) => {
  props.handleTitle("Bencher - Track Your Benchmarks");

  return (
    <section class="section">
      <div class="container">
        <div class="columns">
          <div class="column">
            <LinePlot />
          </div>
        </div>
      </div>
    </section>
  );
};

export default LandingPage;
