import { createFileRoute } from '@tanstack/react-router'
import { OnboardingStep } from './route'
import Illustration from '../../assets/illustrations/onboarding/import_from_others.svg';
import { LAUNCHER_IMPORT_TYPES } from '@/utils';
import LauncherIcon from '@/components/content/LauncherIcon';

export const Route = createFileRoute('/onboarding/import')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <>
      <OnboardingStep illustration={<img src={Illustration} />} paragraph="Import your profiles from other launchers."
        title="Import">
        <div className="h-full w-full flex flex-col gap-y-3">
          <div className="grid grid-cols-3">
            {LAUNCHER_IMPORT_TYPES.map((x) => (
              <button
                className={`flex flex-col items-center justify-center gap-y-4 rounded-md p-4 active:bg-border/10 hover:bg-border/05`}
              >
                <LauncherIcon className="h-16 max-w-22 min-w-16" launcher={x} />
                <span className="text-lg font-medium">{x}</span>
              </button>
            ))}
          </div>
          <small className="pt-2 text-fg-secondary">
            Want to contribute a launcher import? Click
            {' '}
            <a className='text-brand-pressed' href="https://github.com/Polyfrost/OneLauncher">here</a>
            .
          </small>
        </div>
      </OnboardingStep>
    </>
  )
}
