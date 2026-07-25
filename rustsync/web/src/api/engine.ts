import request from '@/utils/request'
import type { ApiResponse, Engine, EngineRequest, StorageMount } from './types'

// ========== 引擎 CRUD ==========

// GET /api/engines
export function listEngines() {
  return request<ApiResponse<Engine[]>>({ url: '/api/engines', method: 'get' })
}

// POST /api/engines
export function addEngine(data: EngineRequest) {
  return request<ApiResponse<null>>({ url: '/api/engines', method: 'post', data })
}

// PUT /api/engines/:id
export function updateEngine(id: number, data: Partial<EngineRequest>) {
  return request<ApiResponse<null>>({ url: `/api/engines/${id}`, method: 'put', data })
}

// DELETE /api/engines/:id
export function deleteEngine(id: number) {
  return request<ApiResponse<null>>({ url: `/api/engines/${id}`, method: 'delete' })
}

// GET /api/engines/:id/browse
export function browseEngine(id: number, path?: string) {
  return request<ApiResponse<{ engine: Engine; children: unknown[]; path: string }>>({
    url: `/api/engines/${id}/browse`,
    method: 'get',
    params: path ? { path } : undefined,
  })
}

// ========== 存储挂载 ==========

// GET /api/storage
export function listStorage(engineId: number) {
  return request<ApiResponse<StorageMount[]>>({ url: '/api/storage', method: 'get', params: { engineId } })
}

// POST /api/storage
export function addStorage(data: Record<string, unknown>) {
  return request<ApiResponse<null>>({ url: '/api/storage', method: 'post', data })
}

// PUT /api/storage/:id
export function updateStorage(id: number, data: Record<string, unknown>) {
  return request<ApiResponse<null>>({ url: `/api/storage/${id}`, method: 'put', data })
}

// DELETE /api/storage/:id
export function deleteStorage(id: number) {
  return request<ApiResponse<null>>({ url: `/api/storage/${id}`, method: 'delete' })
}

// GET /api/storage/local-browse
export function localBrowse(path?: string) {
  return request<ApiResponse<{ path: string; parent?: string; roots: unknown[]; directories: unknown[] }>>({
    url: '/api/storage/local-browse',
    method: 'get',
    params: path ? { path } : undefined,
  })
}

// GET /api/storage/smb-discover
export function smbDiscover() {
  return request<ApiResponse<unknown[]>>({ url: '/api/storage/smb-discover', method: 'get' })
}

// POST /api/storage/sftp-test
export function sftpTest(data: Record<string, unknown>) {
  return request<ApiResponse<{ connected: boolean; message: string }>>({ url: '/api/storage/sftp-test', method: 'post', data })
}

// POST /api/storage/sftp-browse
export function sftpBrowse(data: Record<string, unknown>) {
  return request<ApiResponse<{ path: string; files: unknown[] }>>({ url: '/api/storage/sftp-browse', method: 'post', data })
}