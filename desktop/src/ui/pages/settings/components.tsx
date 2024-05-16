import { type JSX, type ParentProps, createEffect } from 'solid-js';

type SettingsRowProps = ParentProps & {
	title: string;
	description: string;
	icon: JSX.Element;
	clickable?: () => any;
};

function SettingsRow(props: SettingsRowProps) {
	const interactableClass = createEffect(() => props.clickable ? 'hover:bg-component-bg-hover active:bg-component-bg-pressed' : '');

	return (
		<div class={`flex flex-row bg-component-bg rounded-xl gap-4 p-4 ${interactableClass} items-center`}>
			<div class="flex justify-center items-center h-8 w-8">
				{props.icon}
			</div>

			<div class="flex flex-col gap-1 flex-1">
				<h3>{props.title}</h3>
				<p class="text-wrap">{props.description}</p>
			</div>

			<div class="">
				<span>test</span>
			</div>
		</div>
	);
}

export default SettingsRow;
