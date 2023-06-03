const Switch = (props) => {
	return (
		<div class="field" id={props.config?.label}>
			<input
				id={props.config?.label}
				type="checkbox"
				class="switch"
				name={props.config?.label}
				checked={props.value}
				disabled={props.config?.disabled}
			/>
			{/* rome-ignore lint/a11y/useKeyWithClickEvents: TODO */}
			<label
				for={props.config?.label}
				onClick={(_event) => {
					if (props.config?.disabled) {
						return;
					}
					props.handleField(!props.value);
				}}
			/>
		</div>
	);
};

export default Switch;
