# Root Cause Investigation: Ralph Mobile App Not Working

**Investigation Date**: 2026-01-04
**Investigator**: Debugger Agent
**Status**: Phase 1 Complete - Root Cause Identified

## Executive Summary

Ralph mobile app is **properly configured** but **cannot function** because **backend server is not running**.

**Critical Finding**: Mobile app configured to connect to `http://192.168.0.154:8085` but backend is offline.

---

## Investigation Findings

### 1. Mobile App Configuration Status

✅ **App is properly configured:**

- **Entry point**: Uses Expo Router (`index.ts` imports `expo-router/entry`)
- **Navigation**: Tab-based navigation with Dashboard, History, Settings
- **API Client**: Properly configured in `/ralph-mobile/lib/api.ts`
- **State Management**: React Query setup in root layout
- **Authentication**: JWT token storage via SecureStore
- **Theme**: Dark theme matching web UI

### 2. Backend Connection Configuration

**Mobile App Settings** (`/ralph-mobile/.env`):
```
EXPO_PUBLIC_API_URL=http://192.168.0.154:8085
```

**Backend Default Settings** (`/src/ralph_orchestrator/web/server.py:290`):
```python
def __init__(self, host: str = "0.0.0.0", port: int = 8080, enable_auth: bool = True)
```

**Port Configuration** (`/src/ralph_orchestrator/instance.py`):
```python
PORT_RANGE_START = 8080
PORT_RANGE_END = 8180
```

### 3. Connection Flow Analysis

Mobile app → API call sequence:

1. **Dashboard loads** (`app/(tabs)/index.tsx`)
2. **Calls `useOrchestrators` hook** (`hooks/useOrchestrators.ts`)
3. **Fetches from** `${API_BASE_URL}/api/orchestrators`
4. **API_BASE_URL** = `http://192.168.0.154:8085` (from .env)
5. **Backend endpoint expected**: `/api/orchestrators` (auth protected if enabled)

Expected API response format:
```typescript
{
  orchestrators: Orchestrator[],
  total: number
}
```

### 4. Backend Status Check

**Connection Test Results**:
```bash
$ curl -s -m 5 http://192.168.0.154:8085/api/health
CONNECTION FAILED
```

**Port Scan Results**:
```bash
$ lsof -i :8085
(no output - port not in use)

$ lsof -i :8080
(no output - port not in use)
```

**Process Check**:
- No `uvicorn` processes running
- No Ralph web server processes found

### 5. Port Mismatch Analysis

**Critical Discrepancy**:
- Mobile app expects: **Port 8085**
- Backend default: **Port 8080**
- Backend range: **8080-8180**

This mismatch exists but is **NOT the root cause** - backend is simply not running at all.

---

## Root Cause: Backend Server Not Running

### Evidence

1. **No response** from `http://192.168.0.154:8085/api/health`
2. **No processes** listening on port 8085 or 8080
3. **No uvicorn/FastAPI processes** in `ps aux`
4. **Mobile app properly configured** - all code structure correct

### Connection Requirements

Backend server must be started with:
- **Host**: `0.0.0.0` (to accept external connections)
- **Port**: `8085` (to match mobile .env)
- **Auth**: Enabled or disabled (mobile has auth endpoints)

Command to start (based on `/src/ralph_orchestrator/web/__main__.py`):
```bash
python -m ralph_orchestrator.web --port 8085
```

Or from project root:
```bash
uv run python -m ralph_orchestrator.web --port 8085
```

### Expected Error When Running App

When mobile app starts with backend offline:

1. Dashboard screen loads
2. Shows loading spinner
3. `useOrchestrators` hook calls `fetchOrchestrators()`
4. Fetch request times out or returns network error
5. Error state displayed: "Failed to fetch orchestrators"
6. Retry button available

Error message likely:
- "Network request failed" (React Native fetch)
- "Failed to fetch orchestrators" (custom error from API client)

---

## App Architecture Analysis

### Properly Configured Components

**Authentication Flow**:
```
/api/auth/login → POST username/password → JWT token → SecureStore
/api/orchestrators → GET with Bearer token
```

**Data Flow**:
```
Dashboard Screen
  ↓
useOrchestrators hook (auto-refresh every 5s)
  ↓
fetchOrchestrators() API call
  ↓
apiClient.baseURL + "/api/orchestrators"
  ↓
Backend FastAPI server (OFFLINE)
```

**Navigation Structure**:
```
index.ts (expo-router entry)
  ↓
app/_layout.tsx (root layout + React Query)
  ↓
app/(tabs)/_layout.tsx (tab navigation)
  ↓
app/(tabs)/index.tsx (dashboard)
app/(tabs)/history.tsx
app/(tabs)/settings.tsx
```

### Dependencies Status

**Package.json shows**:
- Expo SDK: `~54.0.30`
- React Native: `0.81.5`
- React Navigation: `^7.1.26`
- React Query: `^5.90.16`
- All properly installed in node_modules

---

## Configuration Issues (Secondary)

While not the root cause, these should be noted:

### 1. Port Mismatch
- Mobile: `8085`
- Backend default: `8080`
- Backend must be started with `--port 8085` flag

### 2. IP Address Hardcoded
- Mobile .env: `192.168.0.154` (likely developer's local network IP)
- This IP is **hardcoded** - would break if:
  - Developer connects to different WiFi
  - IP address changes
  - Other team members try to run
  - Production deployment

**Better approach**: Environment-based configuration or service discovery

### 3. Authentication State
- Backend has auth enabled by default
- Mobile app has auth endpoints but no visible login screen flow
- Dashboard expects to be authenticated immediately

---

## Missing Components Analysis

App appears **feature-complete** for current scope:

✅ **Present**:
- Orchestrator list view
- Real-time updates (5s polling)
- Pull-to-refresh
- Error handling
- Loading states
- Navigation structure
- API client with auth
- Theme system
- TypeScript types

❓ **Unclear**:
- Login screen (no route found for auth flow)
- How user authenticates initially
- Token refresh logic

---

## Validation Evidence References

Found in `/validation-evidence/` directory:

1. **phase-05/websocket.txt**: Shows successful mobile connection logs from IP `192.168.0.154`
2. **phase-04/expo-build.txt**: Shows Expo dev client opening on `192.168.0.154:8081`
3. **phase-06/control-api.txt**: Shows API calls to `192.168.0.154:8085`

This indicates the app **previously worked** when backend was running.

---

## Summary: What Would User See?

**When attempting to open the app**:

1. App launches successfully (React Native loads)
2. Shows splash screen
3. Navigates to Dashboard tab
4. Shows "Loading orchestrators..." with spinner
5. After timeout: Shows error message
6. Error: "Failed to fetch orchestrators" with red text
7. "Tap to retry" option available
8. Tapping retry repeats the cycle

**The app itself is not broken** - it's correctly attempting to connect but the backend server doesn't exist at the configured endpoint.

---

## Unresolved Questions

1. **How to start backend?** - Need startup documentation or script
2. **Auth flow incomplete?** - No login screen route found, how does user authenticate?
3. **Production config?** - How is backend URL configured for production/staging?
4. **IP persistence?** - Why is local network IP hardcoded vs localhost or env-specific?

---

## Next Steps (For Fix Phase)

When fixing:

1. Start backend server on port 8085
2. Verify backend health endpoint responds
3. Configure backend to accept connections from mobile network IP
4. Implement login screen if missing
5. Add environment configuration for API URL
6. Test end-to-end connection flow

**Primary Fix**: Start the backend server with `--port 8085`
