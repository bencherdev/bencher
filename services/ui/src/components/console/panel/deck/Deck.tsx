import { For } from "solid-js";

import DeckCard from "./DeckCard";
import DeckButton from "./DeckButton";

const Deck = (props) => {
  return (
    <>
      {props.config?.buttons && (
        <DeckButton
          config={props.config?.buttons}
          path_params={props.path_params}
          handleRedirect={props.handleRedirect}
        />
      )}
      <For each={props.config?.cards}>
        {(card) => (
          <div class="columns">
            <div class="column">
              <div class="card">
                <DeckCard
                  card={card}
                  data={props.data}
                  path_params={props.path_params}
                />
              </div>
            </div>
          </div>
        )}
      </For>
      {props.config?.buttons && (
        <DeckButton
          config={props.config?.buttons}
          path_params={props.path_params}
          handleRedirect={props.handleRedirect}
        />
      )}
    </>
  );
};

export default Deck;
