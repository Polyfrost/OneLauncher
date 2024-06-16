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
		<div class={`flex flex-row bg-component-bg rounded-xl gap-3.5 p-4 ${interactableClass} items-center`}>
			<div class="flex justify-center items-center h-8 w-8">
				{props.icon}
			</div>

			<div class="flex flex-col gap-2 flex-1">
				<h3 class="text-lg">{props.title}</h3>
				<p class="text-wrap text-sm">{props.description}</p>
			</div>

			<div class="">
				{props.children}
			</div>
		</div>
	);
}

export default SettingsRow;
