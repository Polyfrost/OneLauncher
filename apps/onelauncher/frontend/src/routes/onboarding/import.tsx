import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/onboarding/import')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/onboarding/import"!</div>
}
