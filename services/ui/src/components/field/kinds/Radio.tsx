import axios from "axios";
import { createMemo, createResource } from "solid-js";
import { get_options } from "../../site/util";

const Radio = (props) => {
  const radioFetcher = createMemo(() => {
    return {
      url: props.config?.url(props.path_params()),
      token: props.user()?.token,
    };
  });

  const getRadio = async (fetcher) => {
    try {
      const resp = await axios(get_options(fetcher.url, fetcher.token));
      console.log(resp.data);
      return resp.data;
    } catch (error) {
      console.error(error);
      return [];
    }
  };

  const [radio_data] = createResource(radioFetcher, getRadio);

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
              <input type="radio" name={radioFetcher().url} />
              Foo
            </label>
            <br />
            <label class="radio">
              <input type="radio" name={radioFetcher().url} />
              Bar
            </label>
          </div>
        </div>
      </div>
    </nav>
  );
};

export default Radio;
