import { useRef, useState, useCallback } from 'react'
import { useNavigate } from 'react-router'
import { isAnalysisReport } from '../types'

const STORAGE_KEY = 'barad-dur-report'

export default function Landing() {
  const navigate = useNavigate()
  const inputRef = useRef<HTMLInputElement>(null)
  const [dragging, setDragging] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const processFile = useCallback((file: File) => {
    if (!file.name.endsWith('.json') && file.type !== 'application/json') {
      setError('Please drop a .json file.')
      return
    }
    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const text = e.target?.result as string
        const data = JSON.parse(text) as unknown
        if (!isAnalysisReport(data)) {
          setError('Invalid report format. Generate with: barad-dur analyze . --json')
          return
        }
        sessionStorage.setItem(STORAGE_KEY, text)
        void navigate('/report')
      } catch {
        setError('Failed to parse JSON. Make sure the file is valid.')
      }
    }
    reader.readAsText(file)
  }, [navigate])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragging(false)
    setError(null)
    const file = e.dataTransfer.files[0]
    if (file) processFile(file)
  }, [processFile])

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setError(null)
    const file = e.target.files?.[0]
    if (file) processFile(file)
  }

  return (
    <div className="min-h-screen flex flex-col items-center justify-center px-4 py-16 relative overflow-hidden">
      {/* Ambient glow behind center */}
      <div
        style={{
          position: 'absolute',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          width: '600px',
          height: '400px',
          background: 'radial-gradient(ellipse, rgba(245, 158, 11, 0.07) 0%, transparent 70%)',
          pointerEvents: 'none',
        }}
      />

      {/* Header */}
      <div className="text-center mb-16 relative">
        {/* Eye symbol */}
        <div className="mb-6 flex justify-center">
          <div
            style={{
              width: 64,
              height: 64,
              borderRadius: '50%',
              border: '2px solid rgba(245, 158, 11, 0.4)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              boxShadow: '0 0 24px rgba(245, 158, 11, 0.2), inset 0 0 24px rgba(245, 158, 11, 0.05)',
            }}
          >
            <div
              style={{
                width: 20,
                height: 20,
                borderRadius: '50%',
                backgroundColor: '#f59e0b',
                boxShadow: '0 0 16px rgba(245, 158, 11, 0.8), 0 0 32px rgba(245, 158, 11, 0.4)',
              }}
            />
          </div>
        </div>

        <h1
          style={{
            fontFamily: 'Syne, sans-serif',
            fontSize: 'clamp(2.5rem, 6vw, 4rem)',
            fontWeight: 800,
            letterSpacing: '-0.02em',
            lineHeight: 1,
            color: '#f1f5f9',
            marginBottom: '0.5rem',
          }}
        >
          Barad-dûr
        </h1>
        <p
          style={{
            fontFamily: 'JetBrains Mono, monospace',
            fontSize: '0.8rem',
            color: 'rgba(245, 158, 11, 0.7)',
            letterSpacing: '0.2em',
            textTransform: 'uppercase',
          }}
        >
          Repository Intelligence Dashboard
        </p>
      </div>

      {/* Drop Zone */}
      <div
        onDragOver={(e) => { e.preventDefault(); setDragging(true) }}
        onDragLeave={() => setDragging(false)}
        onDrop={handleDrop}
        onClick={() => inputRef.current?.click()}
        style={{
          width: '100%',
          maxWidth: 480,
          border: `2px dashed ${dragging ? 'rgba(245, 158, 11, 0.8)' : 'rgba(255, 255, 255, 0.1)'}`,
          borderRadius: 12,
          padding: '3rem 2rem',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: '1rem',
          cursor: 'pointer',
          backgroundColor: dragging ? 'rgba(245, 158, 11, 0.05)' : 'rgba(255, 255, 255, 0.02)',
          boxShadow: dragging ? '0 0 32px rgba(245, 158, 11, 0.1)' : 'none',
          transition: 'all 0.2s ease',
        }}
      >
        <input
          ref={inputRef}
          type="file"
          accept=".json,application/json"
          onChange={handleFileChange}
          style={{ display: 'none' }}
        />

        {/* Icon */}
        <div style={{ fontSize: 40, opacity: dragging ? 1 : 0.4 }}>
          {dragging ? '📂' : '📄'}
        </div>

        <div className="text-center">
          <p
            style={{
              fontFamily: 'Syne, sans-serif',
              fontWeight: 600,
              fontSize: '1rem',
              color: dragging ? '#f59e0b' : 'rgba(226, 232, 240, 0.8)',
              marginBottom: '0.25rem',
            }}
          >
            {dragging ? 'Release to analyze' : 'Drop your report.json here'}
          </p>
          <p
            style={{
              fontFamily: 'JetBrains Mono, monospace',
              fontSize: '0.7rem',
              color: 'rgba(148, 163, 184, 0.5)',
              letterSpacing: '0.05em',
            }}
          >
            or click to browse
          </p>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div
          style={{
            marginTop: '1rem',
            padding: '0.75rem 1.25rem',
            borderRadius: 8,
            border: '1px solid rgba(239, 68, 68, 0.3)',
            backgroundColor: 'rgba(239, 68, 68, 0.08)',
            color: '#ef4444',
            fontFamily: 'JetBrains Mono, monospace',
            fontSize: '0.75rem',
            maxWidth: 480,
            width: '100%',
            textAlign: 'center',
          }}
        >
          {error}
        </div>
      )}

      {/* Instructions */}
      <div
        style={{
          marginTop: '3rem',
          padding: '1.25rem 1.5rem',
          borderRadius: 10,
          border: '1px solid rgba(255, 255, 255, 0.06)',
          backgroundColor: 'rgba(255, 255, 255, 0.02)',
          maxWidth: 480,
          width: '100%',
        }}
      >
        <p
          style={{
            fontFamily: 'JetBrains Mono, monospace',
            fontSize: '0.7rem',
            color: 'rgba(148, 163, 184, 0.5)',
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
            marginBottom: '0.75rem',
          }}
        >
          Generate a report
        </p>
        <pre
          style={{
            fontFamily: 'JetBrains Mono, monospace',
            fontSize: '0.75rem',
            color: 'rgba(226, 232, 240, 0.7)',
            lineHeight: 1.8,
            margin: 0,
            whiteSpace: 'pre-wrap',
          }}
        >
          {`barad-dur analyze . --json > report.json`}
        </pre>
      </div>
    </div>
  )
}
