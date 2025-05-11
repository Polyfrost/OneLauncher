import { render } from '@solidjs/testing-library';
import Button from './Button';

describe('<Button />', () => {
	it('clicky', async () => {
		const { queryByRole } = render(() => <Button />);
		const button = queryByRole('button') as HTMLButtonElement;
		expect(button).toBeInTheDocument();
	});
});
