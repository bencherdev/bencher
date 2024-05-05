
interface Props {
    text: string;
}

const CopyButton = (props: Props) => {
    return (
        <button
            class="button is-outlined is-fullwidth"
            title="Copy to clipboard"
            onClick={(e) => {
                e.preventDefault();
                navigator.clipboard.writeText(props.text);
            }}
        >
            <span class="icon-text">
                <span class="icon">
                    <i class="far fa-copy"></i>
                </span>
                <span>Copy to Clipboard</span>
            </span>
        </button>
    )
}

export default CopyButton;