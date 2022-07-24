const ConsoleTable = (props) => {
  return (
    <div class="pricing-table is-horizontal">
      <div class="pricing-plan">
        <div class="plan-header">Starter</div>
        <div class="plan-items">
          <div class="plan-item">20GB Storage</div>
          <div class="plan-item">100 Domains</div>
          <div class="plan-item">-</div>
          <div class="plan-item">-</div>
        </div>
        <div class="plan-footer">
          <div class="plan-price">
            <span class="plan-price-amount">
              <span class="plan-price-currency">$</span>20
            </span>
            /month
          </div>
          <button class="button is-fullwidth" disabled={true}>
            Current plan
          </button>
        </div>
      </div>

      <div class="pricing-plan is-warning">
        <div class="plan-header">Startups</div>
        <div class="plan-items">
          <div class="plan-item">20GB Storage</div>
          <div class="plan-item">25 Domains</div>
          <div class="plan-item">1TB Bandwidth</div>
          <div class="plan-item">-</div>
        </div>
        <div class="plan-footer">
          <div class="plan-price">
            <span class="plan-price-amount">
              <span class="plan-price-currency">$</span>40
            </span>
            /month
          </div>
          <button class="button is-fullwidth">Choose</button>
        </div>
      </div>

      <div class="pricing-plan is-active">
        <div class="plan-header">Growing Team</div>
        <div class="plan-items">
          <div class="plan-item">200GB Storage</div>
          <div class="plan-item">50 Domains</div>
          <div class="plan-item">1TB Bandwidth</div>
          <div class="plan-item">100 Email Boxes</div>
        </div>
        <div class="plan-footer">
          <div class="plan-price">
            <span class="plan-price-amount">
              <span class="plan-price-currency">$</span>60
            </span>
            /month
          </div>
          <button class="button is-fullwidth">Choose</button>
        </div>
      </div>

      <div class="pricing-plan is-danger">
        <div class="plan-header">Enterprise</div>
        <div class="plan-items">
          <div class="plan-item">2TB Storage</div>
          <div class="plan-item">100 Domains</div>
          <div class="plan-item">1TB Bandwidth</div>
          <div class="plan-item">1000 Email Boxes</div>
        </div>
        <div class="plan-footer">
          <div class="plan-price">
            <span class="plan-price-amount">
              <span class="plan-price-currency">$</span>100
            </span>
            /month
          </div>
          <button class="button is-fullwidth">Choose</button>
        </div>
      </div>
    </div>
  );
};

export default ConsoleTable;
