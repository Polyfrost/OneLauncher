import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';

function SettingsChangelog() {
	return (
		<Sidebar.Page>
			<h1>Changelog</h1>
			<ScrollableContainer>
				<UpdatePost />
				<UpdatePost />
				<UpdatePost />
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

// TODO: Implement markdown support?
function UpdatePost() {
	return (
		<div class="flex flex-1 flex-col gap-2 whitespace-pre-line rounded-xl bg-component-bg p-4 line-height-relaxed">
			<h2>New Feature</h2>
			<p class="">
				{`
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus fringilla, enim nec lobortis ornare, dui dolor convallis ante, eget blandit dolor purus eget nisi. Nullam malesuada sem purus, nec placerat velit luctus nec. Nullam quis consequat mi. Pellentesque in mi nec ligula rhoncus ullamcorper. Morbi vehicula, magna in lobortis imperdiet, diam orci sodales massa, sed convallis sapien diam id lorem. Cras condimentum tincidunt felis non suscipit. Morbi rhoncus enim nec felis sagittis, sed semper tortor interdum. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Ut sed lacinia est. Etiam a est vitae diam laoreet dapibus. Cras eu dignissim felis. Nam quis magna tempor, convallis nibh eu, sagittis quam.
            
Aliquam ullamcorper libero sem, ac pellentesque nulla gravida eget. Phasellus volutpat lectus sit amet dictum gravida. Vivamus ligula magna, venenatis vitae elementum ut, elementum quis dolor. Pellentesque congue urna nibh, quis mollis nunc sagittis eu. Maecenas lacinia, velit quis euismod pulvinar, eros diam eleifend metus, quis porta nisi ligula ut ipsum. Proin eget diam mi. Cras sed rutrum est, in pellentesque lacus. Donec vitae egestas nibh. Vivamus vulputate convallis dui, vitae tempus velit sagittis eget. Ut sit amet magna a risus ultrices sagittis. Duis imperdiet erat felis, a pretium velit egestas in. Nulla facilisi. Praesent fringilla sem orci, ac elementum magna sollicitudin at. Phasellus convallis gravida est, at venenatis quam maximus id. Cras euismod purus vel massa consequat, sit amet porttitor odio accumsan.
`.trim()}
			</p>
		</div>
	);
}

export default SettingsChangelog;
