import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/onboarding/')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <div className="grid grid-cols-2 h-full w-full items-center gap-x-16">
      <h1 className="text-6xl">
        Welcome to
        {' '}
        <span className="underline underline-8 underline-brand">OneLauncher</span>
      </h1>

      <h3>A powerful yet easy to use launcher for Minecraft.</h3>
    </div>
  )
}
