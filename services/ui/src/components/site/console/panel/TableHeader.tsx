const TableHeader = (props) => {
  return (
    <nav class="level">
      <div class="level-left">
        <div class="level-item">
          <h3 class="title is-3">{props.title}</h3>
        </div>
      </div>

      <div class="level-right">
        <p class="level-item">
          <button class="button is-outlined">
            <span class="icon">
              <i class="fas fa-plus" aria-hidden="true"></i>
            </span>
            <span>Add</span>
          </button>
        </p>
        <p class="level-item">
          <button class="button is-outlined">
            <span class="icon">
              <i class="fas fa-sync-alt" aria-hidden="true"></i>
            </span>
            <span>Refresh</span>
          </button>
        </p>
      </div>
    </nav>
  );
};

export default TableHeader;
