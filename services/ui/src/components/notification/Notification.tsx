import { useSearchParams } from "solid-app-router";
import { createMemo, createSignal, Match, Switch } from "solid-js";
import { NotifyKind } from "../site/util";

const NOTIFY_KIND_PARAM = "notify_kind";
const NOTIFY_TEXT_PARAM = "notify_text";

const Notification = (props) => {
  const [searchParams, setSearchParams] = useSearchParams();

  const isNotifyKind = (kind: any) => {
    switch (parseInt(kind)) {
      case NotifyKind.OK:
      case NotifyKind.ALERT:
      case NotifyKind.ERROR:
        return true;
      default:
        return false;
    }
  };

  const isNotifyText = (text: any) =>
    typeof text === "string" && text.length > 0;

  const removeNotification = () => {
    setSearchParams({
      [NOTIFY_KIND_PARAM]: null,
      [NOTIFY_TEXT_PARAM]: null,
    });
  };

  const handleNotification = (kind: NotifyKind, text: string) => {
    if (isNotifyKind(kind) && isNotifyText(text)) {
      setSearchParams({
        [NOTIFY_KIND_PARAM]: kind,
        [NOTIFY_TEXT_PARAM]: text,
      });
      setTimeout(() => {
        removeNotification();
      }, 4000);
    }
  };

  if (!isNotifyKind(searchParams[NOTIFY_KIND_PARAM])) {
    setSearchParams({ [NOTIFY_KIND_PARAM]: null });
  }

  if (!isNotifyText(searchParams[NOTIFY_TEXT_PARAM])) {
    setSearchParams({ [NOTIFY_TEXT_PARAM]: null });
  }

  const getNotify = () => {
    let color: string;
    switch (parseInt(searchParams[NOTIFY_KIND_PARAM])) {
      case NotifyKind.OK:
        color = "is-success";
        break;
      case NotifyKind.ALERT:
        color = "is-primary";
        break;
      case NotifyKind.ERROR:
        color = "is-danger";
        break;
      default:
        color = "";
    }
    return (
      <div class={`notification ${color}`}>
        🐰 {searchParams[NOTIFY_TEXT_PARAM]}
        <button
          class="delete"
          onClick={(e) => {
            e.preventDefault();
            removeNotification();
          }}
        />
      </div>
    );
  };

  return (
    <div>
      {/* <Switch fallback={<></>}>
        <Match
          when={
            isNotifyKind(searchParams[NOTIFY_KIND_PARAM]) &&
            isNotifyText(searchParams[NOTIFY_TEXT_PARAM])
          }
        >
          <section class="section">
            <div class="container">{getNotify()}</div>
          </section>
        </Match>
      </Switch> */}
    </div>
  );
};

export default Notification;
