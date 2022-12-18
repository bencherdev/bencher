import axios from "axios";
import { createMemo, createResource, For } from "solid-js";
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
      return resp.data;
    } catch (error) {
      console.error(error);
      return [];
    }
  };

  const [data] = createResource(radioFetcher, getRadio);

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
            <For each={data()}>
              {(datum) => (
                <>
                  <label class="radio">
                    <nav class="level is-mobile">
                      <div class="level-left">
                        <div class="level-item">
                          <input
                            type="radio"
                            name={radioFetcher().url}
                            onInput={(_event) =>
                              props.handleField(datum[props.config?.value_key])
                            }
                          />
                        </div>
                        <div class="level-item">
                          {datum[props.config?.option_key]}
                        </div>
                      </div>
                    </nav>
                  </label>
                  <br />
                </>
              )}
            </For>
          </div>
        </div>
      </div>
    </nav>
  );
};

export default Radio;
