const ConsolePage = (props) => {
  props.handleTitle("Bencher Console - Track Your Benchmarks");

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">menu</div>
          <div class="column">panel</div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
