import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/onboarding/summary')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/onboarding/summary"!</div>
}
