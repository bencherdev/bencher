const Radio = (props) => {
  return (
    <nav class="level is-mobile">
      <div class="level-left">
        <div class="level-item">
          <div class="icon is-small is-left">
            <i class={props.config.icon}></i>
          </div>
        </div>
        <div class="level-item">
          <div class="control">
            <label class="radio">
              <input type="radio" name="foobar" />
              Foo
            </label>
            <br />
            <label class="radio">
              <input type="radio" name="foobar" checked />
              Bar
            </label>
          </div>
        </div>
      </div>
    </nav>
  );
};

export default Radio;
