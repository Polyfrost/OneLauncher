import { createFileRoute } from '@tanstack/react-router'
import Sidebar from '../settings/route'
import ScrollableContainer from '@/components/ScrollableContainer'

export const Route = createFileRoute('/app/cluster/worlds')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <Sidebar.Page>
      <h1>Worlds</h1>
      <ScrollableContainer>
        <div className="h-full">
          
        </div>
      </ScrollableContainer>
    </Sidebar.Page>
  )
}
