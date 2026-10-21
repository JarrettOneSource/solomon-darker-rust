// Reuse the Website account forms; the mounted game remains the Rust client.
import { StrictMode, useEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, Navigate, RouterProvider } from 'react-router-dom'
import { AuthProvider } from '@website/lib/auth'
import Login from '@website/pages/Login'
import Register from '@website/pages/Register'
import '@fontsource/cinzel/700.css'
import '@fontsource/alegreya/400.css'
import '@fontsource/alegreya/700.css'
import './account.css'

declare const __GAME_MOUNT__: string

function ReturnToGame() {
  useEffect(() => { window.location.replace(`${__GAME_MOUNT__}/`) }, [])
  return <p className="text-center py-20">Returning to the game…</p>
}

const router = createBrowserRouter([
  { path: '/login', element: <Login /> },
  { path: '/register', element: <Register /> },
  { path: '/account', element: <ReturnToGame /> },
  { path: '*', element: <Navigate to="/login" replace /> },
], { basename: __GAME_MOUNT__ })

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <header className="mx-auto max-w-md px-4 pt-6 text-center">
      <a className="link-arcane" href={`${__GAME_MOUNT__}/`}>Back to the game</a>
      <p className="text-bone-dim mt-2">Staging accounts are separate from the main website.</p>
    </header>
    <AuthProvider><RouterProvider router={router} /></AuthProvider>
  </StrictMode>,
)
