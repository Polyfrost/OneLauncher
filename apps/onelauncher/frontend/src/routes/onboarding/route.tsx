import Button from '@/components/base/Button'
import { createFileRoute, Outlet, useNavigate, useRouterState } from '@tanstack/react-router'

export const Route = createFileRoute('/onboarding')({
  component: RouteComponent,
})

function RouteComponent() {
  const navigate = useNavigate()
  const routerState = useRouterState()
  
  const steps = [
    '/onboarding/',
  ]
  
  const currentPath = routerState.location.pathname
  const currentStepIndex = steps.findIndex(path => currentPath === path || currentPath.startsWith(path))
  
  const progressPercentage = steps.length > 1 
    ? ((currentStepIndex + 1) / steps.length) * 100 
    : (currentStepIndex === 0 ? 100 : 0)
  
  const handleBack = () => {
    if (currentStepIndex > 0) {
      navigate({ to: steps[currentStepIndex - 1] })
    }
  }
  
  const handleNext = () => {
    if (currentStepIndex < steps.length - 1) {
      navigate({ to: steps[currentStepIndex + 1] })
    } else {
      // if completed navigate to main
      navigate({ to: '/app' })
    }
  }
  
  return (
    // remind me 2 hours! i'll fix this
    <div className="w-full flex flex-col items-center h-screen">
      <div className="h-0.5 w-full">
        <div
          className="h-full rounded-lg bg-brand transition-all"
          style={{
            width: `${progressPercentage}%`,
          }}
        />
      </div>

      <div className="flex-1 max-w-280 w-full flex flex-col gap-y-4 p-8">
        <Outlet />
      </div>

      <div className="w-full max-w-280 p-8">
        <div className="w-1/3 flex flex-row items-stretch gap-x-8 [&>*]:w-full ml-auto">
          <Button onClick={handleBack} isDisabled={currentStepIndex === 0}>Back</Button>
          <Button onClick={handleNext}>{currentStepIndex === steps.length - 1 ? 'Finish' : 'Next'}</Button>
        </div>
      </div>
    </div>
  )
}
