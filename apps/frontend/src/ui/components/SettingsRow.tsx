import type { JSX, ParentProps } from 'solid-js';

export type SettingsRowProps = ParentProps & {
	title: JSX.Element;
	description: JSX.Element;
	icon: JSX.Element;
	disabled?: boolean;
};

function SettingsRow(props: SettingsRowProps) {
	return (
		<div
			class="bg-page-elevated flex flex-row items-center gap-3.5 rounded-xl p-4"
			classList={{
				'bg-component-bg-disabled': props.disabled,
				'text-fg-primary-disabled': props.disabled,
			}}
		>
			<div class="h-8 w-8 flex items-center justify-center">
				{props.icon}
			</div>

			<div class="flex flex-1 flex-col gap-2">
				<h3 class="text-lg">{props.title}</h3>
				<p class="text-wrap text-sm">{props.description}</p>
			</div>

			<div class="h-9 flex flex-row items-center justify-center gap-2">
				{props.children}
			</div>
		</div>
	);
}

SettingsRow.Header = (props: JSX.HTMLAttributes<HTMLHeadingElement>) => {
	return <h3 class={`mt-4 mb-1 ml-2 text-md text-fg-secondary uppercase ${props.class || ''}`} {...props} />;
};

export default SettingsRow;
