import Illustration from '@/assets/illustrations/onboarding/language.svg';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/language')({
	component: RouteComponent,
});

// placeholder
interface Language {
	lang: string;
	code: string;
	percentage: number;
}

// placeholder
const languageList: Array<Language> = [
	{
		lang: 'English',
		code: 'en',
		percentage: 100,
	},
];


function RouteComponent() {
	return (
		<div className='flex flex-row h-full p-24 pt-48 gap-8'>
			<div className='h-full w-1/3'>
				<img src={Illustration} />
			</div>

			<div className='flex flex-col w-2/3'>
				<h1 className="text-4xl font-semibold mb-2">Language</h1>
				<p className="text-slate-400 text-lg mb-2">Choose your preferred language.</p>

				<div className="my-4 flex flex-col gap-y-2 rounded-lg bg-page-elevated">
					<div className="flex flex-col gap-y-1 p-2">
						{languageList.map(lang => (
							<div className="flex flex-row items-center rounded-lg px-6 py-5" key={lang.lang}>
								<div className="flex-1 font-medium text-lg ">
									<p>{lang.lang}</p>
								</div>
								<div className="flex-1 text-right text-sm">
									<p>{lang.percentage}%</p>
								</div>
							</div>
						))}
					</div>
				</div>

				<div className="w-full flex flex-row justify-end">
					<p className="text-xs">Help translate OneClient</p>
				</div>

			</div>
		</div>
	);
}
