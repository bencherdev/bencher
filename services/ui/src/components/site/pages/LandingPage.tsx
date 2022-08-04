const LandingPage = (props) => {
  props.handleTitle("Bencher - Track Your Benchmarks");

  return (
    <section class="section is-medium">
      {props.user() && props.handleRedirect("/console")}

      <div class="container">
        <div class="content has-text-centered">
          <h1 class="title">Track your benchmarks</h1>
          <h4 class="subtitle">
            Catch performance regressions before they make it to production
          </h4>
          <div class="columns is-centered">
            <div class="column is-half">
              <button
                class="button is-primary is-large is-responsive is-fullwidth"
                onClick={(e) => {
                  e.preventDefault();
                  props.handleRedirect("/auth/signup");
                }}
              >
                Start Now
              </button>
            </div>
          </div>
        </div>
      </div>

      <hr />

      <section class="section">
        <div class="container">
          <div class="columns is-centered">
            <div class="column">
              <h2 class="title">How It Works</h2>
            </div>
          </div>
          <br />
          <br />
          <div class="columns is-centered">
            <div class="column">
              <div class="columns">
                <div class="column">
                  <div class="content has-text-centered">
                    <span class="icon has-text-primary">
                      <i
                        class="fas fa-tachometer-alt fa-5x"
                        aria-hidden="true"
                      />
                    </span>
                    <h5 class="title">Run your benchmarks</h5>
                  </div>
                  <div class="content">
                    <p>
                      Run your benchmarks locally or in CI using your favorite
                      tools. The <code>bencher</code> CLI simply wraps your
                      existing benchmarking framework and stores its results.
                    </p>
                    <br />
                  </div>
                </div>
              </div>
            </div>
            <br />
            <div class="column">
              <div class="columns">
                <div class="column">
                  <div class="content has-text-centered">
                    <span class="icon has-text-primary">
                      <i class="fas fa-chart-line fa-5x" aria-hidden="true" />
                    </span>
                    <h5 class="title">Track your benchmarks</h5>
                  </div>
                  <div class="content">
                    <p>
                      Track the results of your benchmarks over time. Monitor,
                      query, and graph the results using the Bencher web console
                      based on the source branch and testbed.
                    </p>
                  </div>
                  <br />
                </div>
              </div>
            </div>
            <br />
            <div class="column">
              <div class="columns">
                <div class="column">
                  <div class="content has-text-centered">
                    <span class="icon has-text-primary">
                      <i class="fas fa-bell fa-5x" aria-hidden="true" />
                    </span>
                    <h5 class="title">Catch performance regressions</h5>
                  </div>
                  <div class="content">
                    <p>
                      Catch performance regressions in CI. Bencher uses state of
                      the art, customizable analytics to detect performance
                      regressions before they make it to production.
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </section>
  );
};

export default LandingPage;
