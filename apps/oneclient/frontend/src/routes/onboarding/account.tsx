import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/onboarding/account')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/onboarding/account"!</div>
}
