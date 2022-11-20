import { useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { pageTitle } from "../util";
import validator from "validator";

const PricingPage = (props) => {
  const navigate = useNavigate();

  createEffect(() => {
    // if (props.user().token && validator.isJWT(props.user().token)) {
    //   navigate("/console/");
    // }

    pageTitle("Pricing");
  });

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-mobile">
          <div class="column">
            <div class="content has-text-centered">
              <h1 class="title">Pricing</h1>
              <hr />
              <br />
              <a href="https://calendly.com/bencher/demo" target="_blank">
                🐰 Schedule a free demo
              </a>
              <br />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default PricingPage;
