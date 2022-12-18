const Select = (props) => {
  function getStatus() {
    let selected = props.value.options.find((option) => {
      return props.value.selected === option.value;
    });
    if (selected && selected.status) {
      return (
        <div class="level-right">
          <div class="level-item">{selected.status}</div>
        </div>
      );
    }
  }

  return (
    <nav class="level is-mobile">
      <div class="level-left">
        <div class="level-item">
          <div class="control has-icons-left">
            <div class="icon is-small is-left">
              <i class={props.config.icon}></i>
            </div>
            <div class="select">
              <select
                value={props.value.selected}
                onInput={(event) => props.handleField(event.target?.value)}
              >
                {props.value.options.map((option) => {
                  return (
                    <option id={option.value} value={option.value}>
                      {option.option}
                    </option>
                  );
                })}
              </select>
            </div>
          </div>
        </div>
      </div>
      {getStatus()}
    </nav>
  );
};

export default Select;
