import axios from "axios";
import { For } from "solid-js";

import Card from "./Card";
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
                <Card
                  kind={card.kind}
                  field={card.field}
                  value={props.data?.[card.key]}
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
