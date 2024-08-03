import type { JSX, ParentProps } from 'solid-js';

type SettingsRowProps = ParentProps & {
	title: JSX.Element;
	description: JSX.Element;
	icon: JSX.Element;
	disabled?: boolean;
};

function SettingsRow(props: SettingsRowProps) {
	return (
		<div
			class="flex flex-row bg-component-bg rounded-xl gap-3.5 p-4 items-center"
			classList={{
				'bg-component-bg-disabled': props.disabled,
				'text-fg-primary-disabled': props.disabled,
			}}
		>
			<div class="flex justify-center items-center h-8 w-8">
				{props.icon}
			</div>

			<div class="flex flex-col gap-2 flex-1">
				<h3 class="text-lg">{props.title}</h3>
				<p class="text-wrap text-sm">{props.description}</p>
			</div>

			<div class="flex h-9 flex-row justify-center items-center gap-2">
				{props.children}
			</div>
		</div>
	);
}

SettingsRow.Header = (props: JSX.HTMLAttributes<HTMLHeadingElement>) => {
	return <h3 class={`mt-4 mb-1 ml-2 text-md text-fg-secondary uppercase ${props.class || ''}`} {...props} />;
};

export default SettingsRow;
