const DeckButton = (props) => {
  return (
    <div class="columns">
      <div class="column">
        <div class="box">
          <div class="columns">
            <div class="column">
              <button
                class="button is-fullwidth is-primary"
                onClick={(e) => {
                  e.preventDefault();
                  props.handleClick(props.data);
                }}
              >
                Select
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default DeckButton;
