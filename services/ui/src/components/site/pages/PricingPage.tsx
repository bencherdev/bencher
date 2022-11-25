import { useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { BENCHER_CALENDLY_URL, pageTitle, validate_jwt } from "../util";

const PricingPage = (props) => {
  const navigate = useNavigate();

  createEffect(() => {
    if (validate_jwt(props.user().token)) {
      navigate("/console/");
    }

    pageTitle("Pricing");
  });

  return (
    <div>
      <section class="hero">
        <div class="hero-body">
          <div class="container">
            <div class="columns is-mobile">
              <div class="column">
                <div class="content has-text-centered">
                  <h1 class="title">Pricing</h1>
                  <h3 class="subtitle">
                    Start tracking your benchmarks for free
                  </h3>
                  <a href={BENCHER_CALENDLY_URL} target="_blank">
                    🐰 Schedule a free demo
                  </a>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
      <hr />
      {/* <section class="section">
        <div class="pricing-table is-comparative">
          <div class="pricing-plan is-features">
            <div class="plan-header">Features</div>
            <div class="plan-price">
              <span class="plan-price-amount">
                <small>
                  <small>SaaS or Self-Hosted</small>
                </small>
              </span>
            </div>
            <div class="plan-items">
              <div class="plan-item">Public Projects</div>
              <div class="plan-item">Private Projects</div>
              <div class="plan-item">Team Roles</div>
              <div class="plan-item">Support</div>
              <div class="plan-item">SSO</div>
            </div>
            <div class="plan-footer"></div>
          </div>
          <div class="pricing-plan">
            <div class="plan-header">Free</div>
            <div class="plan-price">
              <span class="plan-price-amount">$0</span>
            </div>
            <div class="plan-items">
              <div class="plan-item" data-feature="Public Projects">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Private Projects">
                <span class="icon is-small">
                  <i class="fas fa-times" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Team Roles">
                <span class="icon is-small">
                  <i class="fas fa-times" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Support">
                <span class="icon is-small">
                  <i class="fas fa-times" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="SSO">
                <span class="icon is-small">
                  <i class="fas fa-times" aria-hidden="true"></i>
                </span>
              </div>
            </div>
            <div class="plan-footer">
              <Link href="/docs/how-to/quick-start">
                <button class="button is-fullwidth">Choose</button>
              </Link>
            </div>
          </div>

          <div class="pricing-plan is-active">
            <div class="plan-header">Team</div>
            <div class="plan-price">
              <span class="plan-price-amount">TBD</span>
            </div>
            <div class="plan-items">
              <div class="plan-item" data-feature="Public Projects">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Private Projects">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Team Roles">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Support">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="SSO">
                <span class="icon is-small">
                  <i class="fas fa-times" aria-hidden="true"></i>
                </span>
              </div>
            </div>
            <div class="plan-footer">
              <a
                class="button is-fullwidth"
                href={BENCHER_CALENDLY_URL}
                target="_blank"
              >
                Contact Us
              </a>
            </div>
          </div>

          <div class="pricing-plan">
            <div class="plan-header">Enterprise</div>
            <div class="plan-price">
              <span class="plan-price-amount">TBD</span>
            </div>
            <div class="plan-items">
              <div class="plan-item" data-feature="Public Projects">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Private Projects">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Team Roles">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="Support">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
              <div class="plan-item" data-feature="SSO">
                <span class="icon is-small has-text-primary">
                  <i class="fas fa-check" aria-hidden="true"></i>
                </span>
              </div>
            </div>
            <div class="plan-footer">
              <a
                class="button is-fullwidth"
                href={BENCHER_CALENDLY_URL}
                target="_blank"
              >
                Contact Us
              </a>
            </div>
          </div>
        </div>
      </section> */}
    </div>
  );
};

export default PricingPage;
