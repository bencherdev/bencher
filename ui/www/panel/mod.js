export function panel(inventory) {
  console.log(inventory);
  const bencher_panel = document.getElementById("bencher-panel");
  bencher_panel.innerHTML = `
  <nav class="panel">
    <p class="panel-heading">
        Bencher
    </p>
    ${(function panel_block() {
      var inventory_blocks = "";
      for (let i = 0; i < inventory.length; i++) {
        inventory_blocks = inventory_blocks.concat(`
          <label class="panel-block">
            <input type="checkbox">
            ${inventory[i]}
          </label>
        `)
      }
      return inventory_blocks;
    })()}
  </nav>
  `;
}