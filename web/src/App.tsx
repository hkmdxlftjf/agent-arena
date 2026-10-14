import { Layout } from "@/components/Layout"
import { TooltipProvider } from "@/components/ui/tooltip"
import { lazy, Suspense } from "react"
import { BrowserRouter, Route, Routes } from "react-router-dom"

const Home = lazy(() => import("@/pages/Home").then((m) => ({ default: m.Home })))
const Games = lazy(() => import("@/pages/Games").then((m) => ({ default: m.Games })))
const Upload = lazy(() => import("@/pages/Upload").then((m) => ({ default: m.Upload })))
const GameDetail = lazy(() => import("@/pages/GameDetail").then((m) => ({ default: m.GameDetail })))
const Leaderboard = lazy(() => import("@/pages/Leaderboard").then((m) => ({ default: m.Leaderboard })))
const UserProfile = lazy(() => import("@/pages/Profile").then((m) => ({ default: m.UserProfile })))
const AgentProfile = lazy(() => import("@/pages/Profile").then((m) => ({ default: m.AgentProfile })))
const MatchPage = lazy(() => import("@/pages/Match").then((m) => ({ default: m.MatchPage })))
const Login = lazy(() => import("@/pages/Login").then((m) => ({ default: m.Login })))
const Agents = lazy(() => import("@/pages/Agents").then((m) => ({ default: m.Agents })))
const Connect = lazy(() => import("@/pages/Connect").then((m) => ({ default: m.Connect })))
const RoomRedirect = lazy(() => import("@/pages/RoomRedirect").then((m) => ({ default: m.RoomRedirect })))

export default function App() {
  return (
    <BrowserRouter>
      <TooltipProvider>
        <Suspense fallback={<div className="mx-auto max-w-6xl px-4 py-8"><div className="h-40 animate-pulse rounded-xl bg-muted" /></div>}>
          <Routes>
            <Route element={<Layout />}>
              <Route path="/" element={<Home />} />
              <Route path="/games" element={<Games />} />
              <Route path="/games/new" element={<Upload />} />
              <Route path="/games/:id" element={<GameDetail />} />
              <Route path="/leaderboard" element={<Leaderboard />} />
              <Route path="/u/:handle" element={<UserProfile />} />
              <Route path="/a/:id" element={<AgentProfile />} />
              <Route path="/m/:id" element={<MatchPage />} />
              <Route path="/r/:code" element={<RoomRedirect />} />
              <Route path="/login" element={<Login />} />
              <Route path="/agents" element={<Agents />} />
              <Route path="/docs/connect" element={<Connect />} />
            </Route>
          </Routes>
        </Suspense>
      </TooltipProvider>
    </BrowserRouter>
  )
}
