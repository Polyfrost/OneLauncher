import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/onboarding/finished')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/onboarding/finished"!</div>
}
