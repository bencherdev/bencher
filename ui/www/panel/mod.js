export function panel(inventory) {
  const bencher_panel = document.getElementById("bencher-panel");
  bencher_panel.innerHTML = `
  <nav class="panel">
    <p class="panel-heading">
        Bencher
    </p>
    <label class="panel-block is-active">
        <input type="checkbox">
        i am active
    </label>
    <label class="panel-block">
        <input type="checkbox">
        i am not active
    </label>
    <label class="panel-block">
        <input type="checkbox">
        i am not active
    </label>
  </nav>
  `;
}