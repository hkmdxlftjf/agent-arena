import { Layout } from "@/components/Layout"
import { TooltipProvider } from "@/components/ui/tooltip"
import { Agents } from "@/pages/Agents"
import { Connect } from "@/pages/Connect"
import { GameDetail } from "@/pages/GameDetail"
import { Games } from "@/pages/Games"
import { Home } from "@/pages/Home"
import { Leaderboard } from "@/pages/Leaderboard"
import { Login } from "@/pages/Login"
import { MatchPage } from "@/pages/Match"
import { AgentProfile, UserProfile } from "@/pages/Profile"
import { BrowserRouter, Route, Routes } from "react-router-dom"

export default function App() {
  return (
    <BrowserRouter>
      <TooltipProvider>
        <Routes>
          <Route element={<Layout />}>
            <Route path="/" element={<Home />} />
            <Route path="/games" element={<Games />} />
            <Route path="/games/:id" element={<GameDetail />} />
            <Route path="/leaderboard" element={<Leaderboard />} />
            <Route path="/u/:handle" element={<UserProfile />} />
            <Route path="/a/:id" element={<AgentProfile />} />
            <Route path="/m/:id" element={<MatchPage />} />
            <Route path="/login" element={<Login />} />
            <Route path="/agents" element={<Agents />} />
            <Route path="/docs/connect" element={<Connect />} />
          </Route>
        </Routes>
      </TooltipProvider>
    </BrowserRouter>
  )
}
