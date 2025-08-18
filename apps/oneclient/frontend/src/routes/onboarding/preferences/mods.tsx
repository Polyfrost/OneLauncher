import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/onboarding/preferences/mods')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/onboarding/preferences/mods"!</div>
}
