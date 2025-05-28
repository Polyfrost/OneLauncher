import { createFileRoute } from '@tanstack/react-router'
import Sidebar from '../settings/route'
import ScrollableContainer from '@/components/ScrollableContainer'
import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'

export const Route = createFileRoute('/app/cluster/settings')({
  component: RouteComponent,
})

function RouteComponent() {
  const { id } = Route.useSearch()

  const result = useCommand("getProfileOrDefault", () => bindings.commands.getProfileOrDefault(id.toString()))

  return (
    <Sidebar.Page>
      <h1>Settings</h1>
      <ScrollableContainer>
        <div className="h-full">
          <pre>{JSON.stringify(result.data, null, 2)}</pre>
        </div>
      </ScrollableContainer>
    </Sidebar.Page>
  )
}
