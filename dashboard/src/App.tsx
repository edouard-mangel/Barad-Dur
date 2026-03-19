import { Routes, Route } from 'react-router'
import Landing from './pages/Landing'
import Report from './pages/Report'

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<Landing />} />
      <Route path="/report" element={<Report />} />
    </Routes>
  )
}
